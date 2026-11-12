import type { ExtensionContext, ReadonlyFooterDataProvider } from "@earendil-works/pi-coding-agent";
import { truncateToWidth, visibleWidth } from "@earendil-works/pi-tui";
import { sep } from "node:path";

const STATE_TYPE = "codex-fast-mode";
const ANSI_RESET = "\x1b[0m";
const contextWindowFormat = new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 2 });

const CLAUDE_ANSI = {
  red: "\x1b[31m",
  green: "\x1b[32m",
  yellow: "\x1b[33m",
  cyan: "\x1b[36m",
  dim: "\x1b[2m",
  brightOrange: "\x1b[1;33m",
} as const;

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

function styleStatus(text: string, ansi: string): string {
  return `${ansi}${text}${ANSI_RESET}`;
}

export function createStatusLine(
  ctx: ExtensionContext,
  footerData: ReadonlyFooterDataProvider,
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

    const modelParts = [accent(sanitizeStatusText(ctx.model?.id ?? "no-model"))];
    if (ctx.model?.reasoning) modelParts.push(styleStatus(ctx.thinkingLevel ?? "off", CLAUDE_ANSI.brightOrange));
    if (ctx.model?.provider === "openai" && process.env.PI_SUBAGENT_CHILD !== "1" && statuses.has(STATE_TYPE)) modelParts.push(styleStatus(
      `fast(${statuses.get(STATE_TYPE) === "on" ? "on" : "off"})`, CLAUDE_ANSI.brightOrange,
    ));
    let model = modelParts.join(border(" · "));
    const contextUsage = ctx.getContextUsage();
    const percent = contextUsage?.percent;
    const contextWindow = contextUsage?.contextWindow ?? ctx.model?.contextWindow;
    const bounded = percent == null ? undefined : Math.max(0, Math.min(100, percent));
    const contextColor = bounded === undefined ? CLAUDE_ANSI.dim
      : bounded >= 90 ? CLAUDE_ANSI.red : bounded >= 70 ? CLAUDE_ANSI.yellow : CLAUDE_ANSI.cyan;
    const contextLabel = styleStatus(bounded === undefined ? "?%" : `${Math.floor(bounded)}%`, contextColor)
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
        return styleStatus("━".repeat(used), contextColor) + border("─".repeat(length - used));
      };
      return segment(0, leftWidth) + label + segment(rightStart, meterWidth - rightStart);
    };
    const scroll = hiddenLines > 0 ? border(`↑ ${hiddenLines} more`) : "";
    const cwd = sanitizeStatusText(formatCwd(ctx.sessionManager.getCwd()));
    const branch = sanitizeStatusText(footerData.getGitBranch() ?? "");
    const cap = (text: string) => theme.style(text, { fg: theme.colors.userMessageBg });
    const identity = (limit: number) => cap("\uE0B6") + theme.bg("userMessageBg",
      ` ${accent(truncateToWidth(cwd, limit, "…"))}${branch ? border(" › ") + theme.fg("text", branch) : ""} `,
    ) + cap("\uE0B0");
    let location = identity(visibleWidth(cwd));
    const extras = [...statuses].filter(([key]) => ![STATE_TYPE, "codex-usage", "bg", "bg-selection"].includes(key))
      .map(([, value]) => sanitizeStatusText(value)).filter(Boolean);
    let usage = sanitizeStatusText(statuses.get("codex-usage") ?? "")
      .replace(/^#+/, (filled) => styleStatus(filled, CLAUDE_ANSI.green));
    const contentWidth = width - 2;
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
      model = truncateToWidth(model, spare, "…");
    }
    if (occupied() > contentWidth) {
      return accent("╭") + truncateToWidth(join([contextLabel, scroll, model]), width - 1, "");
    }
    const meter = renderMeter(contentWidth - surroundingWidth());
    return accent("╭") + join([left(), meter, right()]) + border("─");
  };
}
