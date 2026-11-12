import type { ExtensionAPI, MessageRenderer, Theme, ThemeBg, ToolRenderers } from "@earendil-works/pi-coding-agent";
import { Box, Container, Text, mixColors, rgbColor, truncateToWidth, visibleWidth, type Component, type TuiMouseEvent } from "@earendil-works/pi-tui";

type CallContext = Parameters<NonNullable<ToolRenderers["renderCall"]>>[2];
type ToolResult = Parameters<NonNullable<ToolRenderers["renderResult"]>>[0];

class RoundedFrame implements Component {
  private inset = 0;
  private contentHeight = 0;

  constructor(readonly content: Container | Box, public theme: Theme, public background: ThemeBg) {}

  render(width: number): string[] {
    if (width <= 0) return [];
    this.inset = width >= 5 ? 1 : 0;
    const innerWidth = width - this.inset * 2;
    let lines = this.content.render(innerWidth);
    // Inline graphics carry terminal cursor placement; adding a border would move or corrupt them.
    if (lines.some((line) => line.includes("\x1b_G") || line.includes("\x1b]1337;File="))) {
      this.inset = 0;
      lines = this.content.render(width);
      this.contentHeight = lines.length;
      return lines;
    }
    this.contentHeight = lines.length;
    if (!lines.length) return [];
    const bg = this.theme.getBgAnsi(this.background);
    // Restore the panel background after child resets without replacing deliberate inner backgrounds.
    const fill = (text: string) => this.theme.bg(this.background, text.replace(/\x1b\[(?:0|49)?m/g, `$&${bg}`));
    if (!this.inset) return lines.map((line) => fill(truncateToWidth(line, width, "")));
    const contrast = this.theme.appearance === "light" ? rgbColor(0, 0, 0) : rgbColor(255, 255, 255);
    const color = mixColors(this.theme.colors[this.background], contrast, 0.15, "srgb");
    const border = (text: string) => this.theme.style(text, { fg: color });
    return [
      border(`╭${"─".repeat(innerWidth)}╮`),
      ...lines.map((line) => {
        const text = truncateToWidth(line, innerWidth, "");
        return border("│") + text + " ".repeat(Math.max(0, innerWidth - visibleWidth(text))) + "\x1b[0m" + border("│");
      }),
      border(`╰${"─".repeat(innerWidth)}╯`),
    ].map(fill);
  }

  handleMouse(event: TuiMouseEvent) {
    const x = event.x - this.inset;
    const y = event.y - this.inset;
    const width = event.width - this.inset * 2;
    if (x < 0 || x >= width || y < 0 || y >= this.contentHeight) return undefined;
    return this.content.handleMouse({ ...event, x, y, width, height: this.contentHeight });
  }

  invalidate() {
    this.content.invalidate();
  }
}

type Row = {
  frame: RoundedFrame;
  call?: Component;
  result?: Component;
  callLast?: Component;
  resultLast?: Component;
};

function fallbackCall(name: string, args: unknown, theme: Theme, ctx: CallContext): Component {
  const json = JSON.stringify(args, null, ctx.expanded ? 2 : undefined) ?? "";
  const summary = ctx.expanded ? json : truncateToWidth(json, 120, "…");
  return new Text(`${theme.fg("toolTitle", theme.bold(name))}${summary ? `\n${summary}` : ""}`, 0, 0);
}

function fallbackResult(result: ToolResult, theme: Theme, ctx: CallContext): Component {
  const text = result.content.filter((block) => block.type === "text").map((block) => block.text).join("\n");
  const lines = text ? text.split("\n") : [];
  const shown = ctx.expanded ? lines : lines.slice(0, 10);
  if (shown.length < lines.length) shown.push(`… (${lines.length - shown.length} more lines, ${ctx.keyHint("app.tools.expand", "to expand")})`);
  return new Text(theme.fg(ctx.isError ? "error" : "toolOutput", shown.join("\n")), 0, 0);
}

function rendererFailure(component: Component, slot: string, error: unknown, theme: Theme): Component {
  const content = new Container();
  const reason = String(error instanceof Error ? error.message : error).replace(/[\r\n\t]/g, " ");
  content.addChild(new Text(theme.fg("warning", `Tool ${slot} renderer failed: ${reason}. Showing plain output.`), 0, 0));
  content.addChild(component);
  return content;
}

function singleLine(text: string): Component {
  return { render: (width) => [truncateToWidth(text.replace(/[\r\n\t]/g, " "), width)], invalidate() {} };
}

function diagnosticsTitle(path: unknown, theme: Theme): Component {
  return singleLine(theme.fg("toolTitle", theme.bold("lsp_diagnostics"))
    + (typeof path === "string" ? ` ${theme.fg("muted", path)}` : ""));
}

function diagnosticsSummary(text: string): string {
  const counts = new Map<string, number>();
  for (const match of text.matchAll(/^.+:\d+:\d+ - (error|warning|info|hint|diagnostic):/gm)) {
    counts.set(match[1], (counts.get(match[1]) ?? 0) + 1);
  }
  const parts = ["error", "warning", "info", "hint", "diagnostic"].flatMap((severity) => {
    const count = counts.get(severity) ?? 0;
    return count ? [`${count} ${severity}${count === 1 || severity === "info" ? "" : "s"}`] : [];
  });
  const omitted = [...text.matchAll(/^… (\d+) more diagnostics$/gm)].reduce((sum, match) => sum + Number(match[1]), 0);
  if (omitted) parts.push(`+${omitted} more`);
  // Availability warnings are not a clean diagnostic result. Keep their reason visible.
  const notices = text.split("\n").map((line) => line.trim())
    .filter((line) => /^(?:⚠️|✅)\s/.test(line) && !/^⚠️\s+[^:]+:$/.test(line))
    .map((line) => line.replace(/^(?:⚠️|✅)\s*/, ""));
  parts.push(...notices);
  return parts.join("; ") || text.split("\n").find((line) => line.trim() && line.trim() !== "LSP diagnostics:")?.trim()
    || "No diagnostic output";
}

function diagnosticsBody(text: string, expanded: boolean, hint: string, theme: Theme): Component {
  if (expanded) return new Text(theme.fg("toolOutput", text), 0, 0);
  const summary = theme.fg("toolOutput", diagnosticsSummary(text));
  const suffix = theme.fg("muted", ` (${hint})`);
  return {
    render(width) {
      const available = width - visibleWidth(suffix);
      return [available >= 12 ? truncateToWidth(summary, available) + suffix : truncateToWidth(summary, width)];
    },
    invalidate() {},
  };
}

const diagnosticsRenderers: ToolRenderers = {
  renderCall: (args, theme) => diagnosticsTitle(
    args && typeof args === "object" && "path" in args ? args.path : undefined, theme),
  renderResult: (result, options, theme, ctx) => diagnosticsBody(
    result.content.filter((block) => block.type === "text").map((block) => block.text).join("\n"),
    options.expanded, ctx.keyHint("app.tools.expand", "to expand"), theme),
};

const diagnosticsMessage: MessageRenderer<{ summary?: string; path?: string }> = (message, options, theme) => {
  const text = typeof message.details?.summary === "string" ? message.details.summary
    : typeof message.content === "string" ? message.content
    : message.content.filter((block) => block.type === "text").map((block) => block.text).join("\n");
  const content = new Box(options.outputPad, 0);
  content.addChild(diagnosticsTitle(message.details?.path, theme));
  content.addChild(diagnosticsBody(text, options.expanded, options.keyHint("app.tools.expand", "to expand"), theme));
  return new RoundedFrame(content, theme, "toolSuccessBg");
};

export function roundToolRenderers(name: string, base: ToolRenderers = {}): ToolRenderers {
  if (name === "lsp_diagnostics") base = { ...base, ...diagnosticsRenderers };
  // Pi shares state between the call and result slots. Keep wrapper state separate from the delegate's state.
  const rows = new WeakMap<object, Row>();
  const empty = new Container();
  const getRow = (theme: Theme, ctx: CallContext): Row => {
    const color = ctx.isPartial ? "toolPendingBg" : ctx.isError ? "toolErrorBg" : "toolSuccessBg";
    let row = rows.get(ctx.state);
    if (!row) {
      const content = base.renderShell === "self" ? new Container() : new Box(ctx.outputPad, 0);
      row = { frame: new RoundedFrame(content, theme, color) };
      rows.set(ctx.state, row);
    }
    row.frame.theme = theme;
    row.frame.background = color;
    const content = row.frame.content;
    if (content instanceof Box) {
      content.setPaddingX(ctx.outputPad);
      content.setBgFn((text) => theme.bg(color, text));
    }
    return row;
  };
  const compose = (row: Row) => {
    row.frame.content.clear();
    if (row.call) row.frame.content.addChild(row.call);
    if (row.result) row.frame.content.addChild(row.result);
  };

  return {
    renderShell: "self",
    renderCall(args, theme, ctx) {
      const row = getRow(theme, ctx);
      try {
        row.call = base.renderCall
          ? base.renderCall(args, theme, { ...ctx, lastComponent: row.callLast })
          : fallbackCall(name, args, theme, ctx);
        row.callLast = row.call;
      } catch (error) {
        row.callLast = undefined;
        row.call = rendererFailure(fallbackCall(name, args, theme, ctx), "call", error, theme);
      }
      compose(row);
      return row.frame;
    },
    renderResult(result, options, theme, ctx) {
      const row = getRow(theme, ctx);
      try {
        row.result = base.renderResult
          ? base.renderResult(result, options, theme, { ...ctx, lastComponent: row.resultLast })
          : fallbackResult(result, theme, ctx);
        row.resultLast = row.result;
      } catch (error) {
        row.resultLast = undefined;
        row.result = rendererFailure(fallbackResult(result, theme, ctx), "result", error, theme);
      }
      compose(row);
      // The call component owns both slots so the frame closes once, even while results stream.
      return empty;
    },
  };
}

export default function (pi: ExtensionAPI) {
  pi.registerToolRenderer((name, next) => roundToolRenderers(name, next()));
  pi.registerMessageRenderer("pi-lsp-diagnostics", diagnosticsMessage);
}
