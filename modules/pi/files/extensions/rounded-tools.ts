import type { ExtensionAPI, MessageRendererWrapper, Theme, ThemeColor, ToolRenderers } from "@earendil-works/pi-coding-agent";
import { Box, Container, Text, Spacer, stripTerminalSequences, truncateToWidth, visibleWidth, type Component, type TuiMouseEvent } from "@earendil-works/pi-tui";

type CallContext = Parameters<NonNullable<ToolRenderers["renderCall"]>>[2];
type ToolResult = Parameters<NonNullable<ToolRenderers["renderResult"]>>[0];

class RoundedFrame implements Component {
  private inset = 0;
  private contentHeight = 0;
  // Pi renders the whole transcript every frame; reuse framed lines while the child returns the same strings.
  private cache?: { width: number; style: string; source: string[]; lines: string[] };

  constructor(readonly content: Container | Box, readonly title: string, public theme: Theme, public tone: ThemeColor) {}

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
    const border = (text: string) => this.theme.fg(this.tone, text);
    const side = border("│");
    const style = side;
    const cache = this.cache;
    if (cache && cache.width === width && cache.style === style && cache.source.length === lines.length
      && cache.source.every((line, index) => line === lines[index])) return cache.lines;
    const label = innerWidth >= 7 ? truncateToWidth(this.title, innerWidth - 6, "…") : "";
    const heading = label ? `─── ${label} ` : "";
    const framed = !this.inset ? lines.map((line) => truncateToWidth(line, width, "")) : [
      border(`╭${heading}${"─".repeat(innerWidth - visibleWidth(heading))}╮`),
      ...lines.map((line) => {
        const text = truncateToWidth(line, innerWidth, "");
        const padded = text + " ".repeat(Math.max(0, innerWidth - visibleWidth(text)));
        return side + padded + "\x1b[0m" + side;
      }),
      border(`╰${"─".repeat(innerWidth)}╯`),
    ];
    this.cache = { width, style, source: lines.slice(), lines: framed };
    return framed;
  }

  handleMouse(event: TuiMouseEvent) {
    const x = event.x - this.inset;
    const y = event.y - this.inset;
    const width = event.width - this.inset * 2;
    if (x < 0 || x >= width || y < 0 || y >= this.contentHeight) return undefined;
    return this.content.handleMouse({ ...event, x, y, width, height: this.contentHeight });
  }

  invalidate() {
    this.cache = undefined;
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

function fallbackCall(args: unknown, ctx: CallContext): Component {
  const json = JSON.stringify(args, null, ctx.expanded ? 2 : undefined) ?? "";
  const summary = ctx.expanded ? json : truncateToWidth(json, 120, "…");
  return new Text(summary, 0, 0);
}

function fallbackResult(result: ToolResult, theme: Theme, ctx: CallContext): Component {
  const text = result.content.filter((block) => block.type === "text").map((block) => block.text).join("\n");
  const lines = text ? text.split("\n") : [];
  const shown = ctx.expanded ? lines : lines.slice(0, 10);
  if (shown.length < lines.length) shown.push(`… (${lines.length - shown.length} more lines, ${ctx.keyHint("app.tools.expand", "to expand")})`);
  return new Text((ctx.isError ? theme.fg("error", "×") + " " : "") + theme.fg("toolOutput", shown.join("\n")), 0, 0);
}

function rendererFailure(component: Component, slot: string, error: unknown, theme: Theme): Component {
  const content = new Container();
  const reason = String(error instanceof Error ? error.message : error).replace(/[\r\n\t]/g, " ");
  content.addChild(new Text(theme.fg("warning", `Tool ${slot} renderer failed: ${reason}. Showing plain output.`), 0, 0));
  content.addChild(component);
  return content;
}

function titleOf(name: string): string {
  return stripTerminalSequences(name).replace(/[\u0000-\u001f\u007f-\u009f]/g, " ").trim();
}

const roundMessage: MessageRendererWrapper = (component, message, _options, theme) => {
  if (component instanceof RoundedFrame) return component;
  const content = new Container();
  // Pi's default Markdown shell owns a redundant [type] label and a panel background.
  if (component instanceof Box) {
    const [label, spacer] = component.children;
    if (label instanceof Text && spacer instanceof Spacer &&
      stripTerminalSequences(label.render(visibleWidth(message.customType) + 2)[0] ?? "").trim() === `[${message.customType}]`) {
      component.setBgFn(undefined);
      component.removeChild(label);
      component.removeChild(spacer);
    }
  }
  content.addChild(component);
  return new RoundedFrame(content, titleOf(message.customType), theme, "customMessageLabel");
};

export function roundToolRenderers(name: string, base: ToolRenderers = {}): ToolRenderers {
  // Pi shares state between the call and result slots. Keep wrapper state separate from the delegate's state.
  const rows = new WeakMap<object, Row>();
  const empty = new Container();
  const getRow = (theme: Theme, ctx: CallContext): Row => {
    const color = ctx.isPartial ? "warning" : ctx.isError ? "error" : "success";
    let row = rows.get(ctx.state);
    if (!row) {
      const content = base.renderShell === "self" ? new Container() : new Box(ctx.outputPad, 0);
      row = { frame: new RoundedFrame(content, titleOf(name), theme, color) };
      rows.set(ctx.state, row);
    }
    row.frame.theme = theme;
    row.frame.tone = color;
    const content = row.frame.content;
    if (content instanceof Box) {
      content.setPaddingX(ctx.outputPad);
    }
    return row;
  };
  // Suppress only the delegate's styled heading, never matching words in its rendered output.
  const delegateTheme = (theme: Theme, slot: "call" | "result", isError: boolean): Theme => {
    let firstError = true;
    return Object.assign(Object.create(theme), {
      fg(token: ThemeColor, text: string) {
        if (slot === "call" && token === "toolTitle" &&
          (text === theme.bold(name) || text === theme.bold(name + " "))) return "";
        if (slot === "result" && name === "todo" && /^(?:●|✗) /.test(text))
          return theme.fg(token, text[0]) + text.slice(1);
        if (slot === "result" && name === "read" && isError && firstError && token === "toolOutput" && text) {
          firstError = false;
          return theme.fg("error", "×") + " " + theme.fg(token, text);
        }
        return theme.fg(token, text);
      },
      bg(token: Parameters<Theme["bg"]>[0], text: string) {
        return name === "edit" && ["toolPendingBg", "toolSuccessBg", "toolErrorBg"].includes(token) ? text : theme.bg(token, text);
      },
    });
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
          ? base.renderCall(args, delegateTheme(theme, "call", ctx.isError), { ...ctx, lastComponent: row.callLast })
          : fallbackCall(args, ctx);
        row.callLast = row.call;
      } catch (error) {
        row.callLast = undefined;
        row.call = rendererFailure(fallbackCall(args, ctx), "call", error, theme);
      }
      compose(row);
      return row.frame;
    },
    renderResult(result, options, theme, ctx) {
      const row = getRow(theme, ctx);
      try {
        row.result = base.renderResult
          ? base.renderResult(result, options, delegateTheme(theme, "result", ctx.isError), { ...ctx, lastComponent: row.resultLast })
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
  const supportsMessageWrappers = typeof pi.registerMessageRendererWrapper === "function";
  if (supportsMessageWrappers) {
    pi.registerMessageRendererWrapper(roundMessage);
  } else if (process.env.PI_UNPATCHED !== "1") {
    pi.on("session_start", (_event, ctx) => {
      if (ctx.mode === "tui") ctx.ui.notify("Apply Home Manager and restart Pi to frame all extension messages. Tool frames remain available.", "warning");
    });
  }
}
