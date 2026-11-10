import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";

const SLOT_COUNT = 7;
const REFRESH_MS = 15_000;
const TTL_MS = 45_000;
const UPDATE_MS = 250;
const SOURCE = "pi:herdr-subagents";

type Child = {
  index: number;
  agent: string;
  model: string;
  activity: string;
};

function record(value: unknown): Record<string, unknown> | undefined {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : undefined;
}

function text(value: unknown): string {
  return typeof value === "string"
    ? value.replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, "").replace(/[\x00-\x1f\x7f-\x9f]/g, " ").replace(/\s+/g, " ").trim()
    : "";
}

// pi-subagents projects child progress through the parent tool result.
export function foregroundChildren(details: unknown): Child[] | undefined {
  const progress = record(details)?.progress;
  if (!Array.isArray(progress)) return undefined;
  const children = new Map<number, Child>();
  for (const value of progress) {
    const child = record(value);
    if (!child || !Number.isInteger(child.index) || (child.index as number) < 0) continue;
    if (child.status !== "running" && child.status !== "pending") continue;
    const agent = text(child.agent);
    if (!agent) continue;
    const output = Array.isArray(child.recentOutput)
      ? child.recentOutput.map(text).filter(Boolean).at(-1)
      : undefined;
    const tool = text(child.currentTool);
    const activity = child.activityState === "needs_attention"
      ? "needs attention"
      : child.status === "pending"
        ? `pending: ${text(child.task)}`
        : tool
          ? [tool, text(child.currentPath)].filter(Boolean).join(" ")
          : output || text(child.task) || "working";
    children.set(child.index as number, {
      index: child.index as number,
      agent,
      model: text(child.model) || "model pending",
      activity,
    });
  }
  return [...children.values()].sort((left, right) => left.index - right.index);
}

export function sidebarTokens(children: Child[]): Record<string, string | null> {
  const visible = children.length > SLOT_COUNT ? children.slice(0, SLOT_COUNT - 1) : children;
  const tokens: Record<string, string | null> = {};
  const limit = (value: string) => Array.from(value).slice(0, 80).join("");
  for (let slot = 0; slot < SLOT_COUNT; slot++) {
    const child = visible[slot];
    tokens[`subagent_${slot + 1}`] = child ? limit(`|- ${child.agent} / ${child.model}`) : null;
    tokens[`subagent_${slot + 1}_activity`] = child ? limit(`|  ${child.activity}`) : null;
  }
  if (children.length > SLOT_COUNT) {
    tokens[`subagent_${SLOT_COUNT}`] = `|- +${children.length - visible.length} more subagents`;
  }
  return tokens;
}

export default function (pi: ExtensionAPI): void {
  const paneId = process.env.HERDR_PANE_ID;
  if (process.env.HERDR_ENV !== "1" || !paneId || process.env.PI_SUBAGENT_CHILD === "1") return;
  const binary = process.env.HERDR_BIN_PATH || "herdr";
  const calls = new Map<string, Child[]>();
  let active = false;
  let parentModel: string | null = null;
  let context: ExtensionContext | undefined;
  let refresh: ReturnType<typeof setInterval> | undefined;
  let scheduled: ReturnType<typeof setTimeout> | undefined;
  let sending: Promise<void> | undefined;
  let draining = false;
  let pending: Record<string, string | null> | undefined;
  let warned = false;
  let subscriptions: Array<() => void> = [];

  const tokens = () => ({
    ...sidebarTokens([...calls.values()].flat()),
    pi_model: parentModel,
  });

  const publish = (): Promise<void> => {
    pending = tokens();
    if (draining) return sending!;
    draining = true;
    sending = (async () => {
      while (pending) {
        const snapshot = pending;
        pending = undefined;
        const args = [
          "pane", "report-metadata", paneId,
          "--source", SOURCE,
          "--agent", "pi",
          "--ttl-ms", String(TTL_MS),
        ];
        for (const [key, value] of Object.entries(snapshot)) {
          args.push(...(value === null ? ["--clear-token", key] : ["--token", `${key}=${value}`]));
        }
        try {
          const result = await pi.exec(binary, args, { timeout: 2_000 });
          if (result.code !== 0 || result.killed) {
            throw new Error(text(result.stderr) || `Herdr exited with code ${result.code}`);
          }
          warned = false;
        } catch (error) {
          if (!warned) {
            context?.ui.notify(`Herdr subagents: ${error instanceof Error ? error.message : String(error)}`, "warning");
            warned = true;
          }
        }
      }
      draining = false;
    })();
    return sending;
  };

  const schedule = (): void => {
    if (scheduled) return;
    scheduled = setTimeout(() => {
      scheduled = undefined;
      void publish();
    }, UPDATE_MS);
    scheduled.unref?.();
  };

  const clear = async (): Promise<void> => {
    if (scheduled) clearTimeout(scheduled);
    scheduled = undefined;
    calls.clear();
    await publish();
  };

  pi.on("session_start", async (_event, ctx) => {
    if (ctx.mode !== "tui") return;
    context = ctx;
    active = true;
    parentModel = text(ctx.model?.id) || null;
    await clear();
    subscriptions = [
      pi.events.on("subagent:slash:started", (value: unknown) => {
        const id = record(value)?.requestId;
        if (typeof id === "string") calls.set(`slash:${id}`, []);
      }),
      pi.events.on("subagent:slash:update", (value: unknown) => {
        const id = record(value)?.requestId;
        const children = foregroundChildren(value);
        if (typeof id !== "string" || children === undefined) return;
        calls.set(`slash:${id}`, children);
        schedule();
      }),
      pi.events.on("subagent:slash:response", (value: unknown) => {
        const id = record(value)?.requestId;
        if (typeof id === "string" && calls.delete(`slash:${id}`)) schedule();
      }),
    ];
    refresh = setInterval(() => {
      if (active) void publish();
    }, REFRESH_MS);
    refresh.unref?.();
  });

  pi.on("model_select", async (_event, ctx) => {
    if (!active) return;
    parentModel = text(ctx.model?.id) || null;
    await publish();
  });

  pi.on("tool_execution_start", (event) => {
    if (active && event.toolName === "subagent") calls.set(`tool:${event.toolCallId}`, []);
  });

  pi.on("tool_execution_update", (event) => {
    if (!active || event.toolName !== "subagent") return;
    const children = foregroundChildren(record(event.partialResult)?.details);
    if (children === undefined) return;
    calls.set(`tool:${event.toolCallId}`, children);
    schedule();
  });

  pi.on("tool_execution_end", (event) => {
    if (!active || event.toolName !== "subagent" || !calls.delete(`tool:${event.toolCallId}`)) return;
    schedule();
  });

  pi.on("agent_end", async () => {
    if (!active) return;
    for (const id of calls.keys()) {
      if (id.startsWith("tool:")) calls.delete(id);
    }
    await publish();
  });

  pi.on("session_shutdown", async () => {
    if (!active) return;
    active = false;
    for (const unsubscribe of subscriptions) unsubscribe();
    subscriptions = [];
    if (refresh) clearInterval(refresh);
    refresh = undefined;
    parentModel = null;
    await clear();
    context = undefined;
  });
}
