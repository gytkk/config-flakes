import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";

const STATE_TYPE = "codex-fast-mode";
const DEFAULT_ENABLED = false;

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
