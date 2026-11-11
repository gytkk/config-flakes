import { afterEach, expect, test, vi } from "bun:test";
import install, { applyLifecycle, applyProgress, sidebarTokens, type Child, type Handler } from "../files/extensions/herdr-subagents";

const lifecycle = (id: string, status: string) => ({ id, status, agent: "scout", agentSource: "bundled", index: 0, detached: true });
const progress = (id: string, fields: Record<string, unknown> = {}) => ({
  agent: "scout", agentSource: "bundled", index: 0, task: "private assignment", detached: true,
  progress: { id, status: "running", recentOutput: [], ...fields },
});

test("background children retain terminal status, actual models, and reset on follow-up", () => {
  const children = new Map<string, Child>();
  applyLifecycle(children, lifecycle("Inspect", "started"), 100);
  applyProgress(children, progress("Inspect", {
    resolvedModelIdentity: "provider/actual-model", resolvedModel: "provider/actual-model:high",
    lastIntent: "Reading implementation", currentToolArgs: "secret argument",
  }), 200);
  expect(sidebarTokens(children, 200)).toMatchObject({
    subagent_1_status: "●", subagent_1: "Inspect · provider/actual-model", subagent_1_activity: "Reading implementation",
  });
  applyLifecycle(children, lifecycle("Inspect", "completed"), 300);
  applyProgress(children, progress("Inspect", { recentOutput: ["Final answer"] }), 400);
  expect(sidebarTokens(children, 400)).toMatchObject({ subagent_1_status: "✓", subagent_1_activity: "Final answer" });
  expect(sidebarTokens(children, 5_300).subagent_1).toBeNull();
  applyLifecycle(children, lifecycle("Inspect", "started"), 5_400);
  expect(sidebarTokens(children, 5_400)).toMatchObject({ subagent_1_status: "●", subagent_1: "Inspect", subagent_1_activity: null });
});

test("progress recovers an already running nested child and distinguishes failure and cancellation", () => {
  const children = new Map<string, Child>();
  applyProgress(children, progress("Parent.Child", { lastIntent: "Inspecting child" }), 100);
  applyLifecycle(children, lifecycle("Failed", "failed"), 200);
  applyLifecycle(children, lifecycle("Cancelled", "aborted"), 300);
  expect(sidebarTokens(children, 300)).toMatchObject({
    subagent_1: "Parent.Child", subagent_1_status: "●",
    subagent_2: "Failed", subagent_2_status: "×",
    subagent_3: "Cancelled", subagent_3_status: "■",
  });
});

test("overflow prioritizes running children and sanitizes bounded display text", () => {
  const children = new Map<string, Child>();
  applyLifecycle(children, lifecycle("Finished", "completed"), 100);
  for (let index = 0; index < 8; index++) applyLifecycle(children, lifecycle(`Work${index}`, "started"), 100);
  applyProgress(children, progress("Work0", { lastIntent: `\x1b[31m${"가".repeat(100)}\n\x1b[0m` }), 100);
  const tokens = sidebarTokens(children, 100);
  expect(tokens.subagent_1).toBe("Work0");
  expect(tokens.subagent_1_activity).toBe(`${"가".repeat(79)}…`);
  expect(tokens.subagent_7_status).toBe("+");
  expect(tokens.subagent_7).toBe("3 more subagents");
  expect(tokens.subagent_7_activity).toBeNull();
});

const originalEnv = { ...process.env };
const cleanups: Array<() => Promise<void>> = [];
afterEach(async () => {
  for (const cleanup of cleanups.splice(0)) await cleanup();
  vi.useRealTimers();
  for (const key of Object.keys(process.env)) if (!(key in originalEnv)) delete process.env[key];
  Object.assign(process.env, originalEnv);
});

type Result = { code: number; killed: boolean; stderr: string };
function harness(run?: (args: string[], binary: string) => Promise<Result | void>) {
  const handlers = new Map<string, Handler>();
  const listeners = new Map<string, Set<(event: unknown) => void>>();
  const tokens: Record<string, string | null> = {};
  const labels = new Map<string, string>();
  const reports: string[][] = [];
  const commands: Array<{ binary: string; args: string[] }> = [];
  const warnings: string[] = [];
  const ctx = {
    hasUI: true, agent: { kind: "main" as "main" | "sub" }, model: { id: "parent-model" },
    ui: { notify(message: string) { warnings.push(message); } },
  };
  install({
    on: (name: string, handler: Handler) => handlers.set(name, handler),
    events: { on: (name: string, handler: (event: unknown) => void) => {
      const set = listeners.get(name) ?? new Set();
      listeners.set(name, set); set.add(handler);
      return () => { set.delete(handler); };
    } },
    exec: async (binary: string, args: string[]) => {
      reports.push(args);
      commands.push({ binary, args });
      const result = await run?.(args, binary);
      if (result && (result.code !== 0 || result.killed)) return result;
      const label = args.indexOf("label");
      if (label !== -1) {
        const address = args[label + 1];
        const value = args[label + 2].slice("omp.subagents=".length);
        if (value) labels.set(address, value);
        else labels.delete(address);
      }
      let count = 0;
      for (let i = 0; i < args.length; i++) {
        if (args[i] === "--clear-token") { tokens[args[++i]] = null; count++; }
        else if (args[i] === "--token") {
          const [key, ...value] = args[++i].split("="); tokens[key] = value.join("="); count++;
        }
      }
      expect(count).toBeLessThanOrEqual(16);
      return { code: 0, killed: false, stdout: "", stderr: "" };
    },
  });
  const emit = async (name: string, event: unknown = {}, context = ctx) => { await handlers.get(name)?.(event, context); };
  const bus = (name: string, event: unknown) => { for (const listener of listeners.get(name) ?? []) listener(event); };
  cleanups.push(() => emit("session_shutdown"));
  const snapshot = () => JSON.parse(labels.get(process.env.HC_TERMINAL_ID!)!);
  return { emit, bus, tokens, labels, snapshot, reports, commands, warnings, ctx, listeners };
}

function environment() {
  vi.useFakeTimers();
  process.env.HERDR_ENV = "1";
  process.env.HERDR_PANE_ID = "test:p1";
  process.env.HERDR_SOCKET_PATH = "/unused-test.sock";
  delete process.env.OMPCODE;
  delete process.env.HC_TERMINAL_ID;
  delete process.env.HC_TERMINAL_SOCKET;
  delete process.env.HC_SOCKET;
}


test("parent end preserves background children; replacement and shutdown clear owned metadata", async () => {
  environment();
  const h = harness();
  await h.emit("session_start");
  h.bus("task:subagent:lifecycle", lifecycle("Background", "started"));
  h.bus("task:subagent:progress", progress("Background", { resolvedModelIdentity: "provider/child", lastIntent: "Working" }));
  vi.advanceTimersByTime(300);
  await h.emit("agent_start");
  await h.emit("agent_end");
  expect(h.tokens.subagent_1_status).toBe("●");
  expect(h.tokens.omp_model).toBe("parent-model");
  expect(h.reports.every(args => args[1] === "report-metadata")).toBe(true);
  h.ctx.model = { id: "switched-model" };
  await h.emit("agent_start");
  expect(h.tokens.omp_model).toBe("switched-model");
  await h.emit("session_switch");
  expect(h.tokens.subagent_1).toBeNull();
  await h.emit("session_shutdown");
  expect(h.tokens.omp_model).toBeNull();
  h.bus("task:subagent:lifecycle", lifecycle("Late", "started"));
  vi.advanceTimersByTime(300);
  expect(h.tokens.subagent_1).toBeNull();
});

test("nested shells and headless children cannot overwrite the parent pane", async () => {
  environment();
  process.env.OMPCODE = "1";
  const nested = harness();
  await nested.emit("session_start");
  nested.bus("task:subagent:lifecycle", lifecycle("Nested", "started"));
  delete process.env.OMPCODE;
  const headless = harness();
  await headless.emit("session_start", {}, { ...headless.ctx, hasUI: false });
  headless.bus("task:subagent:lifecycle", lifecycle("Headless", "started"));
  vi.advanceTimersByTime(300);
  expect(nested.reports).toEqual([]);
  expect(headless.reports).toEqual([]);
});

test("shutdown queued behind an in-flight report cannot restore stale children", async () => {
  environment();
  const entered = Promise.withResolvers<void>();
  const gate = Promise.withResolvers<void>();
  let hold = false;
  const h = harness(async () => { if (hold) { hold = false; entered.resolve(); await gate.promise; } });
  await h.emit("session_start");
  hold = true;
  h.bus("task:subagent:lifecycle", lifecycle("Delayed", "started"));
  vi.advanceTimersByTime(300);
  await entered.promise;
  const shutdown = h.emit("session_shutdown");
  gate.resolve();
  await shutdown;
  expect(h.tokens.subagent_1).toBeNull();
  expect(h.tokens.omp_model).toBeNull();
});

test("consecutive TUI session replacements clear every batch even when an old clear is interrupted", async () => {
  environment();
  const entered = Promise.withResolvers<void>();
  const gate = Promise.withResolvers<void>();
  let hold = false;
  const h = harness(async () => {
    if (hold) { hold = false; entered.resolve(); await gate.promise; }
  });
  await h.emit("session_start");
  for (let index = 0; index < 7; index++) h.bus("task:subagent:lifecycle", lifecycle(`Work${index}`, "started"));
  await h.emit("agent_start");
  expect(h.tokens.subagent_7).toBe("Work6");
  hold = true;
  const first = h.emit("session_switch");
  await entered.promise;
  const second = h.emit("session_switch");
  gate.resolve();
  await Promise.all([first, second]);
  for (let index = 1; index <= 7; index++) expect(h.tokens[`subagent_${index}`]).toBeNull();
});

function desktopEnvironment() {
  environment();
  delete process.env.HERDR_ENV;
  delete process.env.HERDR_PANE_ID;
  delete process.env.HERDR_SOCKET_PATH;
  process.env.HC_TERMINAL_ID = "remote/t7";
  process.env.HC_TERMINAL_SOCKET = "/actual core.sock";
  process.env.HC_SOCKET = "/wrong-core.sock";
}

test("desktop native snapshots retain all children, resolved models, expiry and heartbeat without TUI env", async () => {
  desktopEnvironment();
  const h = harness();
  await h.emit("session_start");
  const initial = h.snapshot();
  expect(initial).toMatchObject({ version: 1, model: "parent-model", children: [] });
  for (let index = 0; index < 9; index++) h.bus("task:subagent:lifecycle", lifecycle(`Background${index}`, "started"));
  h.bus("task:subagent:progress", progress("Background0", {
    resolvedModelIdentity: "provider/fallback", resolvedModel: "requested-model",
    lastIntent: `\x1b[31m${"가".repeat(100)}\n`, currentToolArgs: "never expose",
  }));
  h.bus("task:subagent:progress", progress("Background0.Nested", { resolvedModel: "provider/nested", lastIntent: "Reading child" }));
  vi.advanceTimersByTime(300);
  await h.emit("agent_start");
  await h.emit("agent_end");
  expect(h.snapshot().children).toHaveLength(10);
  expect(h.snapshot().children[0]).toEqual({
    id: "Background0", phase: "running", model: "provider/fallback", activity: `${"가".repeat(79)}…`,
  });
  expect(h.snapshot().children[9]).toMatchObject({ id: "Background0.Nested", model: "provider/nested" });
  expect(h.snapshot().session).toBe(initial.session);
  expect(JSON.stringify(h.snapshot())).not.toContain("never expose");
  const statuses = ["completed", "failed", "aborted"];
  for (const [index, status] of statuses.entries()) h.bus("task:subagent:lifecycle", lifecycle(`Background${index}`, status));
  vi.advanceTimersByTime(300);
  await h.emit("agent_start");
  expect(h.snapshot().children.slice(0, 3).map((child: Child) => child.phase)).toEqual(statuses);
  const endedAt = h.snapshot().children[0].endedAt;
  vi.advanceTimersByTime(5_000);
  // Late running progress must not turn an expired completion back into a run.
  h.bus("task:subagent:progress", progress("Background0", { lastIntent: "Late" }));
  await h.emit("agent_start");
  expect(h.snapshot().children).toHaveLength(7);
  expect(h.snapshot().children.some((child: Child) => child.id === "Background0")).toBe(false);
  expect(h.snapshot().updatedAt).toBeGreaterThanOrEqual(endedAt + 5_000);
  const beforeHeartbeat = h.snapshot().updatedAt;
  vi.advanceTimersByTime(5_000);
  await h.emit("agent_start");
  expect(h.snapshot().updatedAt).toBeGreaterThanOrEqual(beforeHeartbeat + 5_000);
  expect(h.commands.every(({ binary, args }) =>
    binary === "env" && args[0] === "HC_SOCKET=/actual core.sock" && args[1] === "herdr-core"
    && args[2] === "label" && args[3] === "remote/t7" && args[4].startsWith("omp.subagents="))).toBe(true);
  await h.emit("session_switch");
  expect(h.snapshot().session).not.toBe(initial.session);
  expect(h.snapshot().children).toEqual([]);
  await h.emit("session_shutdown");
  expect(h.labels.size).toBe(0);
  expect(h.commands.at(-1)?.args.at(-1)).toBe("omp.subagents=");
});

test("desktop replacement and shutdown serialize behind old writes and reject obsolete subscriptions", async () => {
  desktopEnvironment();
  const entered = Promise.withResolvers<void>();
  const gate = Promise.withResolvers<void>();
  let hold = false;
  const h = harness(async () => { if (hold) { hold = false; entered.resolve(); await gate.promise; } });
  await h.emit("session_start");
  const firstSession = h.snapshot().session;
  const oldListener = [...h.listeners.get("task:subagent:lifecycle")!][0];
  hold = true;
  h.bus("task:subagent:lifecycle", lifecycle("Old", "started"));
  vi.advanceTimersByTime(300);
  await entered.promise;
  const shutdown = h.emit("session_shutdown");
  const replacement = h.emit("session_switch");
  oldListener(lifecycle("Obsolete", "started"));
  h.bus("task:subagent:progress", progress("Old", { resolvedModelIdentity: "old/model" }));
  h.bus("task:subagent:lifecycle", lifecycle("Old", "completed"));
  h.bus("task:subagent:progress", progress("New.Nested", { resolvedModelIdentity: "provider/new" }));
  vi.advanceTimersByTime(300);
  gate.resolve();
  await Promise.all([shutdown, replacement]);
  await h.emit("agent_start");
  expect(h.snapshot().session).not.toBe(firstSession);
  expect(h.snapshot().children).toEqual([{ id: "New.Nested", phase: "running", model: "provider/new", activity: "" }]);
  h.bus("task:subagent:lifecycle", lifecycle("Old", "started"));
  await h.emit("agent_start");
  expect(h.snapshot().children[1]).toEqual({ id: "Old", phase: "running", model: "", activity: "" });
  expect([...h.listeners.values()].map(set => set.size)).toEqual([1, 1]);
  h.ctx.model = { id: "new-parent" };
  await h.emit("agent_start");
  expect(h.snapshot().model).toBe("new-parent");
  await h.emit("session_shutdown");
  h.bus("task:subagent:lifecycle", lifecycle("Late", "started"));
  vi.advanceTimersByTime(6_000);
  expect(h.labels.size).toBe(0);
});


test("desktop failures warn once and recover with the current native snapshot", async () => {
  desktopEnvironment();
  let fail = true;
  const h = harness(async () => {
    if (fail) return { code: 1, killed: false, stderr: "\x1b[31mcore unavailable" };
  });
  await h.emit("session_start");
  expect(h.labels.size).toBe(0);
  h.bus("task:subagent:lifecycle", lifecycle("Work", "started"));
  vi.advanceTimersByTime(300);
  await h.emit("agent_start");
  expect(h.warnings).toEqual(["Herdr subagents: core unavailable"]);
  fail = false;
  h.bus("task:subagent:progress", progress("Work", { resolvedModelIdentity: "actual/model" }));
  vi.advanceTimersByTime(300);
  await h.emit("agent_start");
  expect(h.snapshot().children[0]).toMatchObject({ id: "Work", model: "actual/model" });
  expect(h.warnings).toHaveLength(1);
});

test("TUI panes inside core terminals exclusively report pane metadata and recover from exec failure", async () => {
  environment();
  process.env.HC_TERMINAL_ID = "t7";
  process.env.HC_TERMINAL_SOCKET = "/core.sock";
  let fail = true;
  const h = harness(async () => { if (fail) throw new Error("TUI unavailable"); });
  await h.emit("session_start");
  h.bus("task:subagent:lifecycle", lifecycle("Work", "started"));
  vi.advanceTimersByTime(300);
  await h.emit("agent_start");
  expect(h.warnings).toEqual(["Herdr subagents: TUI unavailable"]);
  fail = false;
  h.bus("task:subagent:progress", progress("Work", { resolvedModelIdentity: "actual/model" }));
  vi.advanceTimersByTime(300);
  await h.emit("agent_start");
  expect(h.tokens.subagent_1).toBe("Work · actual/model");
  expect(h.labels.size).toBe(0);
  expect(h.reports.every(args => args[0] === "pane" && args[1] === "report-metadata")).toBe(true);
});

test("desktop excludes nested OMP and clears an active session before headless replacement", async () => {
  desktopEnvironment();
  process.env.OMPCODE = "1";
  const nested = harness();
  await nested.emit("session_start");
  expect(nested.reports).toEqual([]);
  delete process.env.OMPCODE;
  const headless = harness();
  await headless.emit("session_start", {}, { ...headless.ctx, hasUI: false });
  expect(headless.reports).toEqual([]);
  const interactiveChild = harness();
  await interactiveChild.emit("session_start", {}, { ...interactiveChild.ctx, agent: { kind: "sub" } });
  expect(interactiveChild.reports).toEqual([]);
  const parent = harness();
  await parent.emit("session_start");
  expect(parent.labels.size).toBe(1);
  await parent.emit("session_switch", {}, { ...parent.ctx, hasUI: false });
  expect(parent.labels.size).toBe(0);
  expect([...parent.listeners.values()].every(set => set.size === 0)).toBe(true);
});

test("desktop shutdown deletion follows an in-flight snapshot even when the write is killed", async () => {
  desktopEnvironment();
  const entered = Promise.withResolvers<void>();
  const gate = Promise.withResolvers<void>();
  let hold = false;
  const h = harness(async () => {
    if (!hold) return;
    hold = false;
    entered.resolve();
    await gate.promise;
    return { code: 0, killed: true, stderr: "timed out" };
  });
  await h.emit("session_start");
  hold = true;
  h.bus("task:subagent:lifecycle", lifecycle("Delayed", "started"));
  vi.advanceTimersByTime(300);
  await entered.promise;
  const shutdown = h.emit("session_shutdown");
  gate.resolve();
  await shutdown;
  expect(h.labels.size).toBe(0);
  expect(h.commands.at(-1)?.args.at(-1)).toBe("omp.subagents=");
  const count = h.commands.length;
  vi.advanceTimersByTime(20_000);
  expect(h.commands).toHaveLength(count);
});
