import {
  type ExtensionAPI,
  type ExtensionContext,
  type ReadonlyFooterDataProvider,
} from "@earendil-works/pi-coding-agent";
import { CURSOR_MARKER, truncateToWidth, visibleWidth } from "@earendil-works/pi-tui";
import { sep } from "node:path";

const STATE_TYPE = "codex-fast-mode";
const DEFAULT_ENABLED = false;
const ANSI_RESET = "\x1b[0m";

const CLAUDE_ANSI = {
  red: "\x1b[31m",
  yellow: "\x1b[33m",
  cyan: "\x1b[36m",
  dim: "\x1b[2m",
  boldPurple: "\x1b[1;35m",
  brightOrange: "\x1b[1;33m",
} as const;

type FastModeState = {
  enabled: boolean;
};

function isFastModeState(value: unknown): value is FastModeState {
  if (!value || typeof value !== "object") return false;
  return typeof (value as { enabled?: unknown }).enabled === "boolean";
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function isSubagentProcess(): boolean {
  return process.env.PI_SUBAGENT_CHILD === "1";
}

function supportsFastMode(ctx: ExtensionContext): boolean {
  return ctx.model?.provider === "openai" && !isSubagentProcess();
}

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
  pi: ExtensionAPI,
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
    const hasBg = pi.getCommands().some((command) => command.name === "bg");
    const selected = hasBg && statuses.get("bg-selection") === "selected";
    const bgStatus = statuses.get("bg");
    const count = bgStatus === undefined ? "0" : /^bg: (\d+)$/.exec(bgStatus)?.[1] ?? "?";
    let bg = hasBg ? styleStatus(`${selected ? "* " : ""}/bg (${count})`,
      selected ? CLAUDE_ANSI.boldPurple : CLAUDE_ANSI.dim) : "";
    // Keep a visible selection and cursor even when no other status fits.
    if (width < 4) return selected
      ? CURSOR_MARKER + truncateToWidth(bg, width, "")
      : accent("╭") + border("─".repeat(width - 1));

    const modelParts = [accent(sanitizeStatusText(ctx.model?.id ?? "no-model"))];
    if (ctx.model?.reasoning) modelParts.push(styleStatus(ctx.thinkingLevel ?? "off", CLAUDE_ANSI.brightOrange));
    if (supportsFastMode(ctx)) modelParts.push(styleStatus(
      `fast(${statuses.get(STATE_TYPE) === "on" ? "on" : "off"})`, CLAUDE_ANSI.brightOrange,
    ));
    let model = modelParts.join(border(" · "));
    const percent = ctx.getContextUsage()?.percent;
    const bounded = percent == null ? undefined : Math.max(0, Math.min(100, percent));
    const contextColor = bounded === undefined ? CLAUDE_ANSI.dim
      : bounded >= 90 ? CLAUDE_ANSI.red : bounded >= 70 ? CLAUDE_ANSI.yellow : CLAUDE_ANSI.cyan;
    const contextLabel = styleStatus(bounded === undefined ? "?%" : `${Math.floor(bounded)}%`, contextColor);
    const filled = Math.floor((bounded ?? 0) / 10);
    let context = styleStatus("━".repeat(filled), contextColor) + border("─".repeat(10 - filled)) + contextLabel;
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
    let usage = sanitizeStatusText(statuses.get("codex-usage") ?? "");
    const contentWidth = width - 2;
    const left = () => join([location, context, scroll]);
    const right = () => join([bg, ...extras, usage, model]);
    const occupied = () => visibleWidth(left()) + visibleWidth(right()) + 1;

    if (occupied() > contentWidth) location = identity(Math.min(20, visibleWidth(cwd)));
    if (occupied() > contentWidth) {
      const spare = contentWidth - occupied() + visibleWidth(usage);
      usage = spare >= 20 ? truncateToWidth(usage, spare, "…") : "";
    }
    while (occupied() > contentWidth && extras.length) extras.pop();
    if (occupied() > contentWidth) location = "";
    if (occupied() > contentWidth) context = contextLabel;
    if (occupied() > contentWidth && !selected) bg = "";
    if (occupied() > contentWidth) {
      const spare = Math.max(0, contentWidth - visibleWidth(join([context, scroll, bg])) - 4);
      model = truncateToWidth(model, spare, "…");
    }
    if (occupied() > contentWidth) {
      return accent("╭") + (selected ? CURSOR_MARKER : "")
        + truncateToWidth(join([bg, context, scroll, model]), width - 1, "");
    }
    const rightText = right();
    const markedRight = selected ? rightText.replace(bg, CURSOR_MARKER + bg) : rightText;
    const gap = contentWidth - visibleWidth(left()) - visibleWidth(rightText);
    return accent("╭") + left() + border("─".repeat(gap)) + markedRight + border("─");
  };
}

export default function(pi: ExtensionAPI) {
  let enabled = DEFAULT_ENABLED;
  const publish = (ctx: ExtensionContext): void => {
    if (ctx.mode === "tui") ctx.ui.setStatus(STATE_TYPE, enabled ? "on" : "off");
  };
  const setEnabled = (next: boolean, ctx: ExtensionContext): void => {
    enabled = next;
    pi.appendEntry(STATE_TYPE, { enabled } satisfies FastModeState);
    publish(ctx);
  };

  pi.on("session_start", (_event, ctx) => {
    enabled = DEFAULT_ENABLED;
    for (const entry of ctx.sessionManager.getBranch()) {
      if (entry.type !== "custom" || entry.customType !== STATE_TYPE) continue;
      if (isFastModeState(entry.data)) enabled = entry.data.enabled;
    }
    publish(ctx);
  });

  pi.on("before_provider_request", (event, ctx) => {
    if (!enabled || !supportsFastMode(ctx)) return;
    if (!isRecord(event.payload)) return;

    return {
      ...event.payload,
      service_tier: "priority",
    };
  });

  pi.registerCommand("fast", {
    description: "Toggle OpenAI fast mode (on, off, or status)",
    handler: async (args, ctx) => {
      const action = args.trim().toLowerCase();

      if (action === "status") {
        ctx.ui.notify(`OpenAI fast mode is ${enabled ? "on" : "off"}.`, "info");
        return;
      }

      if (action === "" || action === "toggle") {
        setEnabled(!enabled, ctx);
      } else if (action === "on") {
        setEnabled(true, ctx);
      } else if (action === "off") {
        setEnabled(false, ctx);
      } else {
        ctx.ui.notify("Usage: /fast [on|off|status]", "warning");
        return;
      }

      ctx.ui.notify(`OpenAI fast mode ${enabled ? "enabled" : "disabled"}.`, "info");
    },
  });
}
