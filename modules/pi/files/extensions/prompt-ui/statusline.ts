import type { ExtensionContext, ReadonlyFooterDataProvider } from "@earendil-works/pi-coding-agent";
import { mixColors, stripTerminalSequences, truncateToWidth, visibleWidth, type Color } from "@earendil-works/pi-tui";
import { sep } from "node:path";

const STATE_TYPE = "codex-fast-mode";
const contextWindowFormat = new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 2 });

function formatCwd(cwd: string): string {
  const home = process.env.HOME ?? process.env.USERPROFILE;
  if (!home) return cwd;
  if (cwd === home) return "~";
  if (cwd.startsWith(`${home}${sep}`)) return `~${cwd.slice(home.length)}`;
  return cwd;
}

function sanitizeStatusText(text: string): string {
  return text.replace(/[\r\n\t]/g, " ").replace(/ +/g, " ").trim();
}

export function createStatusLine(
  ctx: ExtensionContext,
  footerData: ReadonlyFooterDataProvider,
  getGitStatus: () => string = () => "",
): (width: number, hiddenLines?: number) => string {
  return (width, hiddenLines = 0) => {
    if (width <= 0) return "";
    const theme = ctx.ui.theme;
    const border = (text: string) => theme.fg("dim", text);
    const accent = (text: string) => theme.fg("accent", text);
    const separator = border(" │ ");
    const join = (parts: string[]) => parts.filter(Boolean).join(separator);
    const statuses = footerData.getExtensionStatuses();
    if (width < 4) return accent("╭") + border("─".repeat(width - 1));

    const colors = theme.colors;
    const pathBg = mixColors(colors.userMessageBg, colors.accent, 0.12, "srgb");
    const modelBg = mixColors(colors.userMessageBg, colors.customMessageLabel, 0.12, "srgb");
    // Pull badge text toward the theme foreground for contrast on tinted surfaces.
    const ink = (color: Color) => mixColors(color, colors.text, 0.6, "srgb");
    const badge = (text: string, bg: Color, rightCap = "\uE0B4") => theme.style("\uE0B6", { fg: bg })
      + theme.style(` ${text} `, { bg }) + theme.style(rightCap, { fg: bg });
    const modelId = sanitizeStatusText(ctx.model?.id ?? "no-model");
    const modelParts = [badge(theme.style(modelId, { fg: ink(colors.customMessageLabel), bold: true }), modelBg)];
    if (ctx.model?.reasoning) modelParts.push(theme.getThinkingBorderColor(ctx.thinkingLevel ?? "off")(ctx.thinkingLevel ?? "off"));
    if (ctx.model?.provider === "openai" && process.env.PI_SUBAGENT_CHILD !== "1" && statuses.get(STATE_TYPE) === "fast") modelParts.push(theme.style(
      "fast", { fg: "syntaxKeyword", bold: true },
    ));
    let model = modelParts.join(border(" · "));
    const contextUsage = ctx.getContextUsage();
    const percent = contextUsage?.percent;
    const contextWindow = contextUsage?.contextWindow ?? ctx.model?.contextWindow;
    const bounded = percent == null ? undefined : Math.max(0, Math.min(100, percent));
    const contextColor = bounded === undefined ? "muted"
      : bounded >= 90 ? "error" : bounded >= 70 ? "warning" : "accent";
    const contextLabel = theme.fg(contextColor, bounded === undefined ? "?%" : `${Math.floor(bounded)}%`)
      + (contextWindow != null && Number.isFinite(contextWindow) && contextWindow > 0
        ? border(` / ${contextWindowFormat.format(contextWindow)}`) : "");
    let minimumMeterWidth = Math.max(12, visibleWidth(contextLabel) + 2);
    const renderMeter = (meterWidth: number) => {
      const label = meterWidth >= visibleWidth(contextLabel) + 2 ? ` ${contextLabel} ` : contextLabel;
      const labelWidth = visibleWidth(label);
      const leftWidth = Math.floor((meterWidth - labelWidth) / 2);
      const rightStart = leftWidth + labelWidth;
      const filled = Math.floor(meterWidth * (bounded ?? 0) / 100);
      const segment = (start: number, length: number) => {
        const used = Math.max(0, Math.min(length, filled - start));
        return theme.fg(contextColor, "━".repeat(used)) + border("─".repeat(length - used));
      };
      return segment(0, leftWidth) + label + segment(rightStart, meterWidth - rightStart);
    };
    const scroll = hiddenLines > 0 ? border(`↑ ${hiddenLines} more`) : "";
    const cwd = sanitizeStatusText(formatCwd(ctx.sessionManager.getCwd()));
    const branch = sanitizeStatusText(footerData.getGitBranch() ?? "");
    const gitStatus = branch ? sanitizeStatusText(getGitStatus()) : "";
    const git = gitStatus.split(" ").filter(Boolean).map(part => {
      const color = part === "git!" ? colors.error : part.startsWith("+") ? colors.success
        : part.startsWith("*") ? colors.warning : colors.accent;
      return theme.style(part, { fg: ink(color) });
    }).join(" ");
    const identity = (limit: number) => badge(
      theme.style(stripTerminalSequences(truncateToWidth(cwd, limit, "…")), { fg: ink(colors.accent), bold: true })
        + (branch ? theme.style(" › ", { fg: ink(colors.accent) })
          + theme.style(branch, { fg: ink(colors.customMessageLabel) }) + (git ? ` ${git}` : "") : ""),
      pathBg,
      "\uE0B0",
    );
    let location = identity(visibleWidth(cwd));
    const extras = [...statuses].filter(([key]) => ![STATE_TYPE, "codex-usage"].includes(key))
      .map(([, value]) => sanitizeStatusText(value)).filter(Boolean);
    let usage = sanitizeStatusText(statuses.get("codex-usage") ?? "")
      .replace(/^#+/, (filled) => theme.fg("success", filled));
    const contentWidth = width - 1;
    const left = () => join([location, scroll]);
    const right = () => join([...extras, usage, model]);
    const surroundingWidth = () => visibleWidth(left()) + visibleWidth(right())
      + (left() ? visibleWidth(separator) : 0) + (right() ? visibleWidth(separator) : 0);
    const occupied = () => surroundingWidth() + minimumMeterWidth;

    if (occupied() > contentWidth) location = identity(Math.min(20, visibleWidth(cwd)));
    if (occupied() > contentWidth) {
      const spare = contentWidth - occupied() + visibleWidth(usage);
      usage = spare >= 20 ? truncateToWidth(usage, spare, "…") : "";
    }
    while (occupied() > contentWidth && extras.length) extras.pop();
    if (occupied() > contentWidth) location = "";
    if (occupied() > contentWidth) minimumMeterWidth = visibleWidth(contextLabel);
    if (occupied() > contentWidth) {
      const spare = Math.max(0, contentWidth - occupied() + visibleWidth(model));
      model = truncateToWidth([accent(modelId), ...modelParts.slice(1)].join(border(" · ")), spare, "…");
    }
    if (occupied() > contentWidth) {
      return accent("╭") + truncateToWidth(join([contextLabel, scroll, model]), width - 1, "");
    }
    const meter = renderMeter(contentWidth - surroundingWidth());
    return accent("╭") + join([left(), meter, right()]);
  };
}
