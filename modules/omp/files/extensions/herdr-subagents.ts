import { randomUUID } from "node:crypto";

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
  readonly agent?: { kind: "main" | "sub" };
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
  if (process.env.OMPCODE === "1") return;
  const pane = process.env.HERDR_ENV === "1" && process.env.HERDR_SOCKET_PATH ? process.env.HERDR_PANE_ID : undefined;
  // A TUI can contain several panes inside one core terminal; its pane
  // identity is more specific and must not compete for a shared core label.
  const terminal = pane ? undefined : process.env.HC_TERMINAL_ID;
  if (!pane && !terminal) return;
  const binary = process.env.HERDR_BIN_PATH || "herdr";
  // Core 0.1.20 injects the endpoint with its terminal identity. HC_SOCKET may
  // point elsewhere; never resolve this terminal's address on that other core.
  const socket = process.env.HC_TERMINAL_SOCKET || process.env.HC_SOCKET;
  const children = new Map<string, Child>();
  // A replaced session's late progress is not a new run. A genuine follow-up
  // explicitly emits started before reusing its stable child name.
  const retired = new Set<string>();
  let context: Context | undefined;
  let active = false;
  let session = "";
  let generation = 0;
  let refresh: ReturnType<typeof setInterval> | undefined;
  let scheduled: ReturnType<typeof setTimeout> | undefined;
  let subscriptions: Array<() => void> = [];
  type Publication = { generation: number; tokens: Record<string, string | null>; desktop: string; desktopKey: string };
  let pending: Publication | undefined;
  let sending: Promise<void> | undefined;
  const published = new Map<string, { generation: number; key: string; at: number }>();
  const warned = new Set<string>();

  const execute = async (command: string, args: string[]): Promise<void> => {
    const result = await omp.exec(command, args, { timeout: 2_000 });
    if (result.code !== 0 || result.killed) throw new Error(text(result.stderr) || `Herdr exited with code ${result.code}`);
  };
  const report = async (target: string, key: string, send: () => Promise<void>): Promise<void> => {
    const current = generation;
    const previous = published.get(target);
    if (previous?.generation === current && previous.key === key && Date.now() - previous.at < REFRESH_MS) return;
    try {
      await send();
      if (current !== generation) return;
      published.set(target, { generation: current, key, at: Date.now() });
      warned.delete(target);
    } catch (error) {
      if (!warned.has(target)) context?.ui.notify(`Herdr subagents: ${error instanceof Error ? error.message : String(error)}`, "warning");
      warned.add(target);
    }
  };
  const publish = (): Promise<void> => {
    const now = Date.now();
    const model = active ? limited(text(context?.model?.id)) : "";
    const visible = active ? [...children.values()]
      .filter(child => child.endedAt === undefined || now - child.endedAt < TERMINAL_MS)
      .map(child => ({ ...child, model: limited(child.model), activity: limited(child.activity) })) : [];
    const desktop = active ? { version: 1, session, model, children: visible } : undefined;
    pending = {
      generation,
      tokens: { ...sidebarTokens(children, now), omp_model: model || null },
      desktop: desktop ? JSON.stringify({ ...desktop, updatedAt: now }) : "",
      desktopKey: JSON.stringify(desktop) || "",
    };
    if (sending) return sending;
    sending = (async () => {
      while (pending) {
        const snapshot = pending;
        pending = undefined;
        // One serial queue owns both transports, including session replacement
        // and deletion. An old in-flight write always precedes its replacement.
        if (pane) await report("tui", JSON.stringify(snapshot.tokens), async () => {
          const entries = Object.entries(snapshot.tokens);
          // Herdr limits requests to 16 keys; 15 keeps each child's triplet together.
          for (let offset = 0; offset < entries.length; offset += 15) {
            if (snapshot.generation !== generation) return;
            const args = ["pane", "report-metadata", pane, "--source", SOURCE, "--agent", "omp", "--ttl-ms", String(TTL_MS)];
            for (const [key, value] of entries.slice(offset, offset + 15)) {
              args.push(...(value === null ? ["--clear-token", key] : ["--token", `${key}=${value}`]));
            }
            await execute(binary, args);
          }
        });
        if (terminal && snapshot.generation === generation) await report("desktop", snapshot.desktopKey, async () => {
          // Core labels are free-form strings, with no per-value/row cap in
          // Registry::label (v0.1.20); key= removes only our label.
          const args = ["label", terminal, `omp.subagents=${snapshot.desktop}`];
          await execute(socket ? "env" : "herdr-core", socket ? [`HC_SOCKET=${socket}`, "herdr-core", ...args] : args);
        });
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
    for (const id of children.keys()) retired.add(id);
    children.clear();
  };

  const start: Handler = async (_event, ctx) => {
    const wasActive = active;
    stop();
    const current = ++generation;
    active = ctx.hasUI && ctx.agent?.kind !== "sub";
    context = ctx;
    if (!active) {
      if (wasActive) await publish();
      return;
    }
    session = randomUUID();
    subscriptions = [
      omp.events.on("task:subagent:lifecycle", value => {
        if (current !== generation || !active) return;
        const event = record(value);
        const id = text(event?.id);
        if (event?.status === "started") retired.delete(id);
        else if (retired.has(id)) return;
        applyLifecycle(children, value); schedule();
      }),
      omp.events.on("task:subagent:progress", value => {
        if (current !== generation || !active) return;
        if (retired.has(text(record(record(value)?.progress)?.id))) return;
        applyProgress(children, value); schedule();
      }),
    ];
    // Expires settled rows promptly; unchanged snapshots refresh every 5s.
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
    const current = ++generation;
    active = false;
    stop();
    await publish();
    if (current === generation) context = undefined;
  });
}
