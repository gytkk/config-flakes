const SLOT_COUNT = 7;
const TERMINAL_MS = 5_000;
const REFRESH_MS = 5_000;
const TTL_MS = 15_000;
const SOURCE = "omp:herdr-subagents";
const STATUS = { running: "●", completed: "✓", failed: "×", aborted: "■" };
type Phase = keyof typeof STATUS;

export type Child = {
  id: string;
  phase: Phase;
  model: string;
  activity: string;
  endedAt?: number;
};

type Context = {
  hasUI: boolean;
  readonly model?: { id: string };
  ui: { notify(message: string, type: "warning"): void };
};
export type Handler = (event: unknown, context: Context) => unknown;
// Structural subset of OMP's ExtensionAPI; no runtime package import is needed.
type API = {
  on(event: "session_start" | "session_switch" | "session_shutdown" | "agent_start", handler: Handler): void;
  events: { on(channel: string, handler: (event: unknown) => void): () => void };
  exec(binary: string, args: string[], options: { timeout: number }): Promise<{ code: number; killed: boolean; stderr: string }>;
};

function record(value: unknown): Record<string, unknown> | undefined {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown> : undefined;
}

function text(value: unknown): string {
  return typeof value === "string"
    ? value.replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, "").replace(/[\x00-\x1f\x7f-\x9f]/g, " ").replace(/\s+/g, " ").trim()
    : "";
}

function limited(value: string): string {
  const chars = Array.from(value);
  return chars.length <= 80 ? value : `${chars.slice(0, 79).join("")}…`;
}

function phase(value: unknown): Phase | undefined {
  return value === "running" || value === "completed" || value === "failed" || value === "aborted" ? value : undefined;
}

// Native event contracts: OMP 18.18.2 task:subagent:{lifecycle,progress}.
export function applyLifecycle(children: Map<string, Child>, value: unknown, now = Date.now()): void {
  const event = record(value);
  const id = text(event?.id);
  const next = event?.status === "started" ? "running" : phase(event?.status);
  if (!id || !next) return;
  if (next === "running") {
    children.set(id, { id, phase: next, model: "", activity: "" });
    return;
  }
  const child = children.get(id) ?? { id, phase: next, model: "", activity: "" };
  if (child.endedAt === undefined) child.endedAt = now;
  child.phase = next;
  children.set(id, child);
}

export function applyProgress(children: Map<string, Child>, value: unknown, now = Date.now()): void {
  const progress = record(record(value)?.progress);
  const id = text(progress?.id);
  const next = phase(progress?.status);
  if (!id || !progress || !next) return;
  const child = children.get(id) ?? { id, phase: next, model: "", activity: "" };
  // Late progress cannot resurrect a settled run. A follow-up emits started first.
  if (child.endedAt === undefined) {
    child.phase = next;
    if (next !== "running") child.endedAt = now;
  }
  child.model = text(progress.resolvedModelIdentity) || text(progress.resolvedModel) || child.model;
  const output = Array.isArray(progress.recentOutput) ? progress.recentOutput.map(text).filter(Boolean).reverse().join(" ") : "";
  child.activity = (child.phase === "running" ? text(progress.lastIntent) || output : output)
    || text(progress.currentTool) || child.activity;
  children.set(id, child);
}

export function sidebarTokens(children: Map<string, Child>, now = Date.now()): Record<string, string | null> {
  const visible = [...children.values()]
    .filter(child => child.endedAt === undefined || now - child.endedAt < TERMINAL_MS)
    .sort((left, right) => Number(left.phase !== "running") - Number(right.phase !== "running"));
  const count = visible.length > SLOT_COUNT ? SLOT_COUNT - 1 : SLOT_COUNT;
  const tokens: Record<string, string | null> = {};
  for (let index = 0; index < SLOT_COUNT; index++) {
    const child = index < count ? visible[index] : undefined;
    const key = `subagent_${index + 1}`;
    tokens[`${key}_status`] = child ? STATUS[child.phase] : null;
    tokens[key] = child ? limited([child.id, child.model].filter(Boolean).join(" · ")) : null;
    tokens[`${key}_activity`] = child?.activity ? limited(child.activity) : null;
  }
  if (visible.length > SLOT_COUNT) {
    tokens.subagent_7_status = "+";
    tokens.subagent_7 = `${visible.length - count} more subagents`;
  }
  return tokens;
}

export default function (omp: API): void {
  const pane = process.env.HERDR_PANE_ID;
  if (process.env.HERDR_ENV !== "1" || !pane || !process.env.HERDR_SOCKET_PATH || process.env.OMPCODE === "1") return;
  const binary = process.env.HERDR_BIN_PATH || "herdr";
  const children = new Map<string, Child>();
  let context: Context | undefined;
  let active = false;
  let refresh: ReturnType<typeof setInterval> | undefined;
  let scheduled: ReturnType<typeof setTimeout> | undefined;
  let subscriptions: Array<() => void> = [];
  let pending: Record<string, string | null> | undefined;
  let sending: Promise<void> | undefined;
  let lastPublished = "";
  let publishedAt = 0;
  let warned = false;

  const publish = (): Promise<void> => {
    pending = { ...sidebarTokens(children), omp_model: active ? limited(text(context?.model?.id)) || null : null };
    if (sending) return sending;
    sending = (async () => {
      while (pending) {
        const snapshot = pending;
        pending = undefined;
        const serialized = JSON.stringify(snapshot);
        if (serialized === lastPublished && Date.now() - publishedAt < REFRESH_MS) continue;
        try {
          const entries = Object.entries(snapshot);
          // Herdr limits requests to 16 keys; 15 keeps each child's triplet together.
          for (let offset = 0; offset < entries.length; offset += 15) {
            const args = ["pane", "report-metadata", pane, "--source", SOURCE, "--agent", "omp", "--ttl-ms", String(TTL_MS)];
            for (const [key, value] of entries.slice(offset, offset + 15)) {
              args.push(...(value === null ? ["--clear-token", key] : ["--token", `${key}=${value}`]));
            }
            const result = await omp.exec(binary, args, { timeout: 2_000 });
            if (result.code !== 0 || result.killed) throw new Error(text(result.stderr) || `Herdr exited with code ${result.code}`);
          }
          lastPublished = serialized;
          publishedAt = Date.now();
          warned = false;
        } catch (error) {
          if (!warned) context?.ui.notify(`Herdr subagents: ${error instanceof Error ? error.message : String(error)}`, "warning");
          warned = true;
        }
      }
    })().finally(() => { sending = undefined; });
    return sending;
  };

  const schedule = (): void => {
    if (scheduled) return;
    scheduled = setTimeout(() => { scheduled = undefined; void publish(); }, 250);
    scheduled.unref?.();
  };

  const stop = (): void => {
    clearInterval(refresh);
    clearTimeout(scheduled);
    refresh = undefined;
    scheduled = undefined;
    for (const unsubscribe of subscriptions) unsubscribe();
    subscriptions = [];
    children.clear();
  };

  const start: Handler = async (_event, ctx) => {
    if (!ctx.hasUI) return;
    stop();
    context = ctx;
    active = true;
    subscriptions = [
      omp.events.on("task:subagent:lifecycle", value => { applyLifecycle(children, value); schedule(); }),
      omp.events.on("task:subagent:progress", value => { applyProgress(children, value); schedule(); }),
    ];
    // Also expires terminal rows and observes idle model switches through the context getter.
    refresh = setInterval(() => { void publish(); }, 1_000);
    refresh.unref?.();
    await publish();
  };
  omp.on("session_start", start);
  omp.on("session_switch", start);
  omp.on("agent_start", async (_event, ctx) => {
    if (!active) return;
    context = ctx;
    await publish();
  });
  omp.on("session_shutdown", async () => {
    if (!active) return;
    active = false;
    stop();
    await publish();
    context = undefined;
  });
}
