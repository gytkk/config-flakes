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

function harness(run?: (args: string[]) => Promise<void>) {
  const handlers = new Map<string, Handler>();
  const listeners = new Map<string, Set<(event: unknown) => void>>();
  const tokens: Record<string, string | null> = {};
  const reports: string[][] = [];
  const ctx = { hasUI: true, model: { id: "parent-model" }, ui: { notify() {} } };
  install({
    on: (name: string, handler: Handler) => handlers.set(name, handler),
    events: { on: (name: string, handler: (event: unknown) => void) => {
      const set = listeners.get(name) ?? new Set();
      listeners.set(name, set); set.add(handler);
      return () => { set.delete(handler); };
    } },
    exec: async (_binary: string, args: string[]) => {
      reports.push(args);
      await run?.(args);
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
  return { emit, bus, tokens, reports, ctx };
}

function environment() {
  vi.useFakeTimers();
  process.env.HERDR_ENV = "1";
  process.env.HERDR_PANE_ID = "test:p1";
  process.env.HERDR_SOCKET_PATH = "/unused-test.sock";
  delete process.env.OMPCODE;
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
