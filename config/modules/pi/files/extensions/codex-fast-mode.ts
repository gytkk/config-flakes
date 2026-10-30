import type { Usage } from "@earendil-works/pi-ai";
import {
  type ExtensionAPI,
  type ExtensionContext,
} from "@earendil-works/pi-coding-agent";
import { truncateToWidth, visibleWidth } from "@earendil-works/pi-tui";
import { sep } from "node:path";

const STATE_TYPE = "codex-fast-mode";
const DEFAULT_ENABLED = true;
const HORIZONTAL_PADDING = 1;
const ANSI_RESET = "\x1b[0m";

const CLAUDE_ANSI = {
  red: "\x1b[31m",
  green: "\x1b[32m",
  yellow: "\x1b[33m",
  cyan: "\x1b[36m",
  dim: "\x1b[2m",
  boldBlue: "\x1b[1;34m",
  brightOrange: "\x1b[1;33m",
} as const;

type FastModeState = {
  enabled: boolean;
};

type TokenTotals = {
  input: number;
  output: number;
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

function addUsage(totals: TokenTotals, usage: Usage): void {
  totals.input += usage.input;
  totals.output += usage.output;
}

export function createTokenTotalsReader(): (ctx: ExtensionContext) => TokenTotals {
  let cached: {
    manager: ExtensionContext["sessionManager"];
    sessionId: string;
    leafId: string | null;
    totals: TokenTotals;
  } | undefined;

  return (ctx) => {
    const manager = ctx.sessionManager;
    const sessionId = manager.getSessionId();
    const leafId = manager.getLeafId();
    // Entries are append-only and every append moves the leaf.
    if (
      cached &&
      cached.manager === manager &&
      cached.sessionId === sessionId &&
      cached.leafId === leafId
    ) {
      return cached.totals;
    }

    const totals: TokenTotals = { input: 0, output: 0 };
    for (const entry of manager.getEntries()) {
      if (entry.type === "usage") {
        addUsage(totals, entry.usage);
      } else if (entry.type === "message") {
        if (entry.message.role === "assistant") {
          addUsage(totals, entry.message.usage);
        } else if (entry.message.role === "toolResult" && entry.message.usage) {
          addUsage(totals, entry.message.usage);
        }
      } else if (
        (entry.type === "branch_summary" || entry.type === "compaction") &&
        entry.usage
      ) {
        addUsage(totals, entry.usage);
      }
    }

    cached = { manager, sessionId, leafId, totals };
    return totals;
  };
}

function formatTokens(count: number): string {
  if (count < 1_000) return count.toString();
  if (count < 1_000_000) {
    const value =
      count < 10_000 ? (count / 1_000).toFixed(1) : Math.round(count / 1_000);
    return `${value}k`;
  }

  const value =
    count < 10_000_000
      ? (count / 1_000_000).toFixed(1)
      : Math.round(count / 1_000_000);
  return `${value}M`;
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

export default function (pi: ExtensionAPI) {
  let enabled = DEFAULT_ENABLED;
  let requestRender: (() => void) | undefined;

  const installFooter = (ctx: ExtensionContext): void => {
    if (ctx.mode !== "tui") return;

    const getTokenTotals = createTokenTotalsReader();
    ctx.ui.setFooter((tui, _theme, footerData) => {
      requestRender = () => tui.requestRender();
      const unsubscribe = footerData.onBranchChange(() => tui.requestRender());

      return {
        render: (width: number): string[] => {
          const sidePadding =
            width >= HORIZONTAL_PADDING * 2 ? HORIZONTAL_PADDING : 0;
          const contentWidth = width - sidePadding * 2;
          if (contentWidth <= 0) return [" ".repeat(width)];

          const separator = ` ${styleStatus("|", CLAUDE_ANSI.dim)} `;
          const sections: string[] = [];

          sections.push(
            styleStatus(formatCwd(ctx.sessionManager.getCwd()), CLAUDE_ANSI.cyan),
          );

          const branch = footerData.getGitBranch();
          if (branch) sections.push(sanitizeStatusText(branch));

          const extensionStatuses = footerData.getExtensionStatuses();
          const modelParts = [sanitizeStatusText(ctx.model?.id ?? "no-model")];
          if (ctx.model?.reasoning) {
            modelParts.push(
              styleStatus(ctx.thinkingLevel ?? "off", CLAUDE_ANSI.brightOrange),
            );
          }
          if (supportsFastMode(ctx)) {
            modelParts.push(
              styleStatus(
                `fast(${enabled ? "on" : "off"})`,
                CLAUDE_ANSI.brightOrange,
              ),
            );
          }
          const modelSeparator = ` ${styleStatus("·", CLAUDE_ANSI.dim)} `;
          const modelStatus = modelParts.join(modelSeparator);

          const contextUsage = ctx.getContextUsage();
          const percent = contextUsage?.percent;
          if (percent === null || percent === undefined) {
            sections.push(`${styleStatus("----------", CLAUDE_ANSI.dim)} ?%`);
          } else {
            const boundedPercent = Math.max(0, Math.min(100, percent));
            const filled = Math.floor((boundedPercent * 10) / 100);
            const contextColor =
              boundedPercent >= 90
                ? CLAUDE_ANSI.red
                : boundedPercent >= 70
                  ? CLAUDE_ANSI.yellow
                  : CLAUDE_ANSI.green;
            const bar =
              styleStatus("#".repeat(filled), contextColor) +
              styleStatus("-".repeat(10 - filled), CLAUDE_ANSI.dim);
            sections.push(`${bar} ${Math.floor(boundedPercent)}%`);
          }

          const totals = getTokenTotals(ctx);
          sections.push(
            `${styleStatus("tokens", CLAUDE_ANSI.dim)} ${styleStatus(`↓${formatTokens(totals.input)}`, CLAUDE_ANSI.boldBlue)} ${styleStatus(`↑${formatTokens(totals.output)}`, CLAUDE_ANSI.brightOrange)}`,
          );

          for (const status of extensionStatuses.values()) {
            const styled = sanitizeStatusText(status);
            if (styled) sections.push(styled);
          }

          const left = sections.join(separator);
          const modelWidth = visibleWidth(modelStatus);
          const ellipsis = styleStatus("...", CLAUDE_ANSI.dim);
          const margin = " ".repeat(sidePadding);
          if (modelWidth + 2 > contentWidth) {
            const truncatedModel = truncateToWidth(modelStatus, contentWidth, ellipsis);
            const modelPadding = " ".repeat(
              contentWidth - visibleWidth(truncatedModel),
            );
            return [margin + modelPadding + truncatedModel + margin];
          }

          const availableLeft = contentWidth - modelWidth - 2;
          const truncatedLeft = truncateToWidth(left, availableLeft, ellipsis);
          const padding = " ".repeat(
            contentWidth - visibleWidth(truncatedLeft) - modelWidth,
          );
          return [margin + truncatedLeft + padding + modelStatus + margin];
        },
        invalidate: () => {},
        dispose: () => {
          requestRender = undefined;
          unsubscribe();
        },
      };
    });
  };

  const setEnabled = (next: boolean): void => {
    enabled = next;
    pi.appendEntry(STATE_TYPE, { enabled } satisfies FastModeState);
    requestRender?.();
  };

  pi.on("session_start", (_event, ctx) => {
    enabled = DEFAULT_ENABLED;
    for (const entry of ctx.sessionManager.getBranch()) {
      if (entry.type !== "custom" || entry.customType !== STATE_TYPE) continue;
      if (isFastModeState(entry.data)) enabled = entry.data.enabled;
    }
    installFooter(ctx);
  });

  pi.on("model_select", () => {
    requestRender?.();
  });

  pi.on("thinking_level_select", () => {
    requestRender?.();
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
        setEnabled(!enabled);
      } else if (action === "on") {
        setEnabled(true);
      } else if (action === "off") {
        setEnabled(false);
      } else {
        ctx.ui.notify("Usage: /fast [on|off|status]", "warning");
        return;
      }

      ctx.ui.notify(`OpenAI fast mode ${enabled ? "enabled" : "disabled"}.`, "info");
    },
  });
}
