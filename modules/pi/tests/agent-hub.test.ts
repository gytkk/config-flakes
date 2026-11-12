import { afterEach, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdtemp, mkdir, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { KeybindingsManager, TUI_KEYBINDINGS, stripTerminalSequences, visibleWidth } from "@earendil-works/pi-tui";
import agentHub from "../files/extensions/agent-hub";
import { branchOrigins, callerTask, childKey, completionObservation, foregroundRows, mergeChild, OBSERVATION, RESUME, restoreRecords, resumedRun, snapshotObservation, TASK_UNAVAILABLE } from "../files/extensions/agent-hub/records";
import { HubRpc, REPLY, REQUEST } from "../files/extensions/agent-hub/rpc";
import { createViewer, displayText } from "../files/extensions/agent-hub/view";
import { acceptanceEvidence, completionText } from "../files/extensions/agent-hub/completion";

const owner = { sessionId: "parent-a", sessionFile: "/sessions/parent-a.jsonl" };
const result = (index = 0) => ({ index, agent: "worker", task: "[prompt redacted]", exitCode: 0, model: "openai/gpt-6.1-sol", sessionFile: "/sessions/child.jsonl", finalOutput: "Implementation done", usage: { input: 20, output: 30, cacheRead: 0, cacheWrite: 0, cost: 0, turns: 1 } });
const details = (rows = [result()]) => ({ mode: rows.length > 1 ? "parallel" : "single", runId: "exact-run-123", results: rows });
const callEntry = (args: unknown = { agent: "worker", task: "Implement exactly this task" }, id = "call-a") => ({ type: "message", message: { role: "assistant", content: [{ type: "toolCall", name: "subagent", id, arguments: args }] } });
const toolEntry = (value: unknown = details(), id = "call-a") => ({ type: "message", message: { role: "toolResult", toolName: "subagent", toolCallId: id, details: value } });
const slashStart = (id = "slash-id") => ({ type: "custom_message", customType: "subagent-slash-result", details: { requestId: id, result: { details: { mode: "single", results: [{ index: 0, agent: "worker", task: "Slash task" }] } } } });
const plainTheme = { fg: (_: string, text: string) => text };
const keys = new KeybindingsManager(TUI_KEYBINDINGS);
class Bus {
  handlers = new Map<string, Set<(value: unknown) => void>>();
  on(name: string, handler: (value: unknown) => void) {
    const handlers = this.handlers.get(name) ?? new Set();
    handlers.add(handler); this.handlers.set(name, handlers);
    return () => { handlers.delete(handler); if (!handlers.size) this.handlers.delete(name); };
  }
  emit(name: string, value: unknown) { for (const handler of [...(this.handlers.get(name) ?? [])]) handler(value); }
}
const stops: Array<() => void> = [];
afterEach(() => stops.splice(0).forEach(stop => stop()));
function fixture(entries: any[] = [callEntry(), toolEntry()]) {
  let branch = entries;
  let allEntries = entries;
  let sessionOwner = { ...owner };
  let forked = false;
  const bus = new Bus();
  const handlers = new Map<string, Function>();
  let command: Function;
  const requests: any[] = [];
  const notifications: string[] = [];
  const screens: string[] = [];
  const selections: Array<string | number | undefined> = [];
  const confirms: boolean[] = [];
  const inputs: Array<string | undefined> = [];
  const titles: string[] = [];
  const ctx = {
    mode: "tui", hasUI: true,
    sessionManager: { getSessionId: () => sessionOwner.sessionId, getSessionFile: () => sessionOwner.sessionFile, getBranch: () => branch, getEntries: () => allEntries, getHeader: () => forked ? { parentSession: "/sessions/original" } : {} },
    ui: {
      select: async (title: string, options: string[]) => { titles.push(title); const answer = selections.shift(); return typeof answer === "number" ? options[answer] : answer; },
      input: async () => inputs.shift(),
      confirm: async (title: string, body: string) => { screens.push(`${title}\n${body}`); return confirms.shift() ?? false; },
      notify: (text: string) => notifications.push(text),
      custom: async (factory: Function) => {
        let ended = false;
        const component = factory({ terminal: { rows: 30 }, requestRender() {} }, plainTheme, keys, () => { ended = true; });
        screens.push(component.render(120).join("\n"));
        component.handleInput("\x1b");
        expect(ended).toBe(true);
        component.dispose?.();
      },
    },
  };
  const respond = (request: any, data: unknown) => bus.emit(REPLY + request.requestId, { version: 1, requestId: request.requestId, method: request.method, success: true, data });
  let response = (request: any) => respond(request, request.method === "ping" ? { version: 1, session: owner, capabilities: { resume: true } } : request.method === "resume" ? { text: "Revived as background run", details: { mode: "single", results: [], asyncId: "new-exact-run" } } : { text: "Run exact-run-123\nState: completed", details: { mode: "single", results: [] } });
  bus.on(REQUEST, (request: any) => { requests.push(request); response(request); });
  agentHub({
    on: (name: string, handler: Function) => handlers.set(name, handler), events: bus,
    registerCommand: (name: string, options: { handler: Function }) => { expect(name).toBe("agent-hub"); command = options.handler; },
    appendEntry: (customType: string, data: unknown) => {
      const entry = { type: "custom", customType, data: structuredClone(data) };
      allEntries.push(entry);
      if (branch !== allEntries) branch.push(entry);
    },
  } as never);
  const stop = () => handlers.get("session_shutdown")?.({}, ctx);
  stops.push(stop);
  handlers.get("session_start")!({}, ctx);
  return { bus, ctx, entries, handlers, requests, notifications, screens, selections, confirms, inputs, titles, stop,
    setBranch: (next: any[]) => { branch = next; handlers.get("session_tree")!({}, ctx); },
    replace: (nextOwner: typeof owner, next: any[] = [], fork = false) => { stop(); sessionOwner = nextOwner; allEntries = next; branch = next; forked = fork; handlers.get("session_start")!({}, ctx); },
    command: () => command!("", ctx), respond, setResponse: (fn: typeof response) => { response = fn; } };
}

test("restores real tool and wrapped slash result shapes with stable numeric indexes", () => {
  const children = restoreRecords([callEntry(), toolEntry(details([result(9), result(2)])), slashStart(), { type: "custom_message", customType: "subagent-slash-result", details: { requestId: "slash-id", result: { content: [], details: { ...details(), runId: "slash-run" } } } }], owner, false).children;
  expect([...children.values()].map(row => [row.runId, row.index])).toEqual([["exact-run-123", 9], ["exact-run-123", 2], ["slash-run", 0]]);
  expect([...children.values()][0].status).toBe("completed");
  expect(foregroundRows({ ...details(), background: true }, true)).toEqual([]);
  expect(foregroundRows(details([{ ...result(), runner: { type: "external-cli" } }] as any), true)).toEqual([]);
  expect(foregroundRows(details([{ ...result(), runId: "nested" }] as any), true)).toEqual([]);
  expect(foregroundRows(details([{ ...result(), index: undefined }] as any), true)).toEqual([]);
  expect(foregroundRows(details([{ ...result(), detached: true }] as any), true)[0].status).toBe("detached");
  expect(foregroundRows(details([result(0), result(0)]), true)).toEqual([]);
  expect(foregroundRows({ ...details([result(0), result(1)]), mode: "single" }, true)).toEqual([]);
});

test("fork restoration requires exact owner observation; foreign receipts are excluded", () => {
  const observation = (session: unknown) => ({ type: "custom", customType: OBSERVATION, data: { owner: session, runId: "exact-run-123", child: foregroundRows(details(), true)[0] } });
  expect(restoreRecords([toolEntry()], owner, true).children.size).toBe(0);
  expect(restoreRecords([observation({ ...owner, sessionId: "other" }), toolEntry()], owner, true).children.size).toBe(0);
  expect(restoreRecords([observation(owner), toolEntry()], owner, true).children.size).toBe(1);
  const child = foregroundRows(details(), true)[0];
  const receipt = { type: "custom", customType: RESUME, data: { owner: { ...owner, sessionFile: "/other" }, requestId: "r", target: child, state: "accepted", resumedRunId: "new-run" } };
  expect(restoreRecords([receipt], owner, false).children.size).toBe(0);
});

test("selection, details and cancel are read-only; no RPC or parent prompt", async () => {
  const f = fixture();
  f.selections.push(0, 0, undefined, undefined);
  await f.command();
  expect(f.requests).toEqual([]);
  expect(f.screens.join("\n")).toContain("Implementation done");
  expect(f.screens.join("\n")).toContain("Child index: 0");
});

test("transcript uses exact run/index and bounded package view; never resumes", async () => {
  const f = fixture(); f.selections.push(0, 2, undefined, undefined);
  await f.command();
  expect(f.requests.map(r => r.method)).toEqual(["ping", "status"]);
  expect(f.requests[1].params).toEqual({ id: "exact-run-123", index: 0, view: "transcript", lines: 500 });
});

test.each([undefined, "", "   "])("cancelled/empty follow-up %s cannot execute", async message => {
  const f = fixture(); f.selections.push(0, 3, undefined, undefined); f.inputs.push(message);
  await f.command();
  expect(f.requests.some(r => r.method === "resume")).toBe(false);
});

test("explicit confirmation alone dispatches one detached resume and persists correlation", async () => {
  const f = fixture(); f.selections.push(0, 3, undefined); f.inputs.push("Check the edge case"); f.confirms.push(true);
  await f.command();
  const calls = f.requests.filter(r => r.method === "resume");
  expect(calls).toHaveLength(1);
  expect(calls[0].params).toEqual({ id: "exact-run-123", index: 0, message: "Check the edge case" });
  expect(f.screens.join("\n")).toContain("NEW detached background child");
  expect(f.screens.join("\n")).toContain("not completed");
  const state = restoreRecords(f.entries, owner, false);
  expect(state.receipts.get(childKey(foregroundRows(details(), true)[0]))?.requestId).toBe(calls[0].requestId);
  expect(state.children.get('["new-exact-run",0]')?.source).toBe("hub-resume");
  expect(state.children.get('["new-exact-run",0]')?.task).toBe("Check the edge case");
  // Opening a restored receipt is inspection, not another launch.
  f.stop(); f.handlers.get("session_start")!({}, f.ctx);
  f.selections.push(0, 0, undefined, undefined); await f.command();
  expect(f.requests.filter(r => r.method === "resume")).toHaveLength(1);
});

test("declined confirmation, missing capability and wrong owner never resume", async () => {
  const f = fixture(); f.selections.push(0, 3, undefined, undefined); f.inputs.push("Follow up"); f.confirms.push(false);
  await f.command();
  expect(f.requests.some(r => r.method === "resume")).toBe(false);
  for (const data of [ { version: 1, session: { ...owner, sessionId: "foreign" }, capabilities: { resume: true } }, { version: 1, session: owner, capabilities: {} } ]) {
    const g = fixture(); g.setResponse(r => g.respond(r, r.method === "ping" ? data : { text: "Completed" }));
    g.selections.push(0, 3, undefined, undefined); await g.command();
    expect(g.requests.some(r => r.method === "resume")).toBe(false);
    expect(g.notifications.length).toBeGreaterThan(0);
  }
});

test("shutdown during confirmation cannot send; no stale UI access after replacement", async () => {
  const f = fixture(); f.selections.push(0, 3); f.inputs.push("Follow up");
  f.ctx.ui.confirm = async () => { f.stop(); return true; };
  await f.command();
  expect(f.requests.some(r => r.method === "resume")).toBe(false);
});

test("branch navigation invalidates an open confirmation without execution", async () => {
  const f = fixture(); f.selections.push(0, 3); f.inputs.push("Follow up");
  f.ctx.ui.confirm = async () => { f.handlers.get("session_tree")!({}, f.ctx); return true; };
  await f.command();
  expect(f.requests.some(r => r.method === "resume")).toBe(false);
});

test("shutdown after dispatch keeps unresolved receipt and ignores a late success", async () => {
  const f = fixture();
  f.setResponse(r => {
    if (r.method === "resume") { f.stop(); f.respond(r, { details: { asyncId: "late-child" } }); }
    else f.respond(r, r.method === "ping" ? { version: 1, session: owner, capabilities: { resume: true } } : { text: "Completed" });
  });
  f.selections.push(0, 3); f.inputs.push("Follow up"); f.confirms.push(true); await f.command();
  const state = restoreRecords(f.entries, owner, false);
  expect([...state.receipts.values()][0].state).toBe("requested");
  expect(state.children.has('["late-child",0]')).toBe(false);
  expect([...f.bus.handlers.keys()].filter(name => name.startsWith(REPLY))).toEqual([]);
});

test("package rejection is distinct from transport uncertainty across reload, never retried", async () => {
  const f = fixture();
  f.setResponse(r => {
    if (r.method === "resume") f.bus.emit(REPLY + r.requestId, { version: 1, requestId: r.requestId, success: false, error: { code: "execution_failed", message: "Missing recovery metadata" } });
    else f.respond(r, r.method === "ping" ? { version: 1, session: owner, capabilities: { resume: true } } : { text: "Completed" });
  });
  f.selections.push(0, 3, undefined, undefined); f.inputs.push("Follow up"); f.confirms.push(true); await f.command();
  expect(f.notifications.join("\n")).toContain("Missing recovery metadata");
  expect([...restoreRecords(f.entries, owner, false).receipts.values()][0].state).toBe("rejected");
  f.stop(); f.handlers.get("session_start")!({}, f.ctx);
  f.selections.push(0, 3, undefined, undefined); await f.command();
  expect(f.requests.filter(r => r.method === "resume")).toHaveLength(1);
});

test("live progress and terminal results preserve identity, slash unknown responses ignored", async () => {
  const f = fixture([]);
  f.handlers.get("tool_execution_start")!({ toolName: "subagent", toolCallId: "live-call", args: { agent: "worker", task: "Live task" } }, f.ctx);
  f.handlers.get("tool_execution_update")!({ toolCallId: "live-call", toolName: "subagent", partialResult: { details: details([{ ...result(2), progress: { status: "running" } }] as any) } }, f.ctx);
  f.selections.push(0, undefined, undefined); await f.command();
  expect(f.titles.join("\n")).toContain("running");
  f.handlers.get("tool_execution_end")!({ toolCallId: "live-call", toolName: "subagent", result: { details: details([result(2)]) } }, f.ctx);
  f.bus.emit("subagent:slash:response", { requestId: "unseen", result: { details: { ...details(), runId: "foreign" } } });
  f.bus.emit("subagent:slash:started", { requestId: "slash" });
  f.bus.emit("subagent:slash:response", { requestId: "slash", result: { details: { ...details(), runId: "slash-native" } } });
  f.selections.push(undefined); await f.command();
  expect([...new Set(f.entries.filter(e => e.customType === OBSERVATION).map(e => e.data.runId))]).toEqual(["exact-run-123", "slash-native"]);
});

test("RPC ignores unrelated correlation, times out missing API and detaches late replies", async () => {
  const bus = new Bus(); const controller = new AbortController(); const rpc = new HubRpc(bus as never, controller.signal);
  const promise = rpc.request("ping", {}, "request-a", 5);
  bus.emit(REPLY + "request-a", { version: 1, requestId: "wrong-id", success: true, data: {} });
  await expect(promise).rejects.toThrow("API unavailable");
  expect(bus.handlers.size).toBe(0);
  bus.emit(REPLY + "request-a", { version: 1, requestId: "request-a", success: true, data: {} });
  const pending = rpc.request("status", {}, "request-b", 100);
  controller.abort(); await expect(pending).rejects.toThrow("session ended");
  expect(bus.handlers.size).toBe(0);
});

test("resume timeout warns of possible execution and removes reply listener", async () => {
  const bus = new Bus(); const rpc = new HubRpc(bus as never, new AbortController().signal);
  await expect(rpc.request("resume", { id: "run", index: 0, message: "hello" }, "resume-attempt", 5)).rejects.toThrow("may still be running");
  expect(bus.handlers.size).toBe(0);
});

test("viewer uses native width wrapping and scroll state, strips terminal controls", () => {
  let closed = 0;
  const viewer = createViewer("Native child", "한글 wide\n" + Array.from({ length: 80 }, (_, i) => `line ${i}`).join("\n"), plainTheme as never, keys, () => 20, () => {}, () => closed++);
  for (const width of [1, 3, 20, 80]) {
    const lines = viewer.render(width);
    expect(lines.length).toBeLessThanOrEqual(16);
    expect(lines.every(line => visibleWidth(line) <= width)).toBe(true);
  }
  viewer.handleInput!("\x1b[6~");
  expect(viewer.render(80).map(stripTerminalSequences).join("\n")).not.toContain("한글");
  viewer.invalidate();
  viewer.handleInput!("\x1b"); expect(closed).toBe(1);
  expect(displayText("\x1b]52;c;secret\x07text\x1b[31m red\x1b[0m")).toBe("text red");
});

test("Node/jiti discovers only agent-hub entry point, no tools or launcher", async () => {
  const root = await mkdtemp(join(tmpdir(), "agent-hub-loader-"));
  try {
    const agent = join(root, "agent"); const cwd = join(root, "project");
    await mkdir(join(agent, "extensions"), { recursive: true }); await mkdir(cwd);
    await symlink(fileURLToPath(new URL("../files/extensions/agent-hub", import.meta.url)), join(agent, "extensions/agent-hub"));
    const loaded = spawnSync("node", ["--input-type=module", "-e", `
      import { pathToFileURL } from 'node:url';
      const { discoverAndLoadExtensions } = await import(pathToFileURL(process.argv[1]).href);
      const result = await discoverAndLoadExtensions([], process.argv[2], process.env.PI_CODING_AGENT_DIR);
      console.log(JSON.stringify({ errors: result.errors, extensions: result.extensions.map(e => ({ commands: [...e.commands.keys()], tools: [...e.tools.keys()] })) }));
    `, fileURLToPath(import.meta.resolve("@earendil-works/pi-coding-agent")), cwd], { encoding: "utf8", timeout: 30000, env: { ...process.env, PI_CODING_AGENT_DIR: agent, PI_SUBAGENT_CHILD: "0", JITI_FS_CACHE: "0" } });
    expect(loaded.status).toBe(0);
    expect(JSON.parse(loaded.stdout)).toEqual({ errors: [], extensions: [{ commands: ["agent-hub"], tools: [] }] });
  } finally { await rm(root, { recursive: true, force: true }); }
}, 60000);

const tick = () => new Promise<void>(resolve => setTimeout(resolve, 0));
const acceptedReceipt = (id = "hub-run", target = foregroundRows(details(), true)[0]) => ({ type: "custom", customType: RESUME, data: { owner, requestId: `request-${id}`, target, followUp: "Follow-up task", state: "accepted", resumedRunId: id } });
const nativeSnapshot = (id: string, state: string, extra: any = {}) => ({ kind: "pi-subagents.async-status-snapshot", version: 1, generatedAt: 1000, caps: { maxRuns: 20 }, omitted: { runs: 0, children: 0, byteLimitExceeded: false }, runs: [{ id, kind: "subagent", label: "worker", state, children: [{ id: "step:0", kind: "step", agent: "worker", model: "openai/resolved", state }], ...extra }] });
const nativeNotice = (id = "hub-run", status = "completed") => ({ runs: [{ runId: id, agent: "worker", status, source: "async", sessionLabel: "Session file", sessionValue: "/sessions/revived.jsonl" }] });
const childFrom = (f: ReturnType<typeof fixture>, id = "hub-run") => restoreRecords(f.entries, owner, false).children.get(childKey({ runId: id, index: 0 }));

// Priority-1 regressions use the package's redacted terminal task and public
// parent call/start records, not an unredacted mock terminal result.
test("redacted tasks correlate exact source call and numeric child index, not result order", () => {
  const entries = [
    callEntry({ tasks: [{ agent: "worker", task: "Repeated", count: 2 }, { agent: "worker", task: "Third" }] }),
    callEntry({ agent: "worker", task: "Other call" }, "call-b"),
    toolEntry({ ...details([result(2), result(0), result(1)]), mode: "parallel" }),
    toolEntry({ ...details(), runId: "other-run" }, "call-b"),
  ];
  const children = [...restoreRecords(entries, owner, false).children.values()];
  expect(children.map(child => child.task)).toEqual(["Third", "Repeated", "Repeated", "Other call"]);
  expect(callerTask({ agent: "worker", task: "Wrong index" }, 7, "worker")).toBeUndefined();
  expect(callerTask({ chain: [{ agent: "worker", task: "{previous}" }] }, 0, "worker")).toBeUndefined();
  expect(callerTask({ tasks: [{ agent: "worker", task: "Bad count", count: 1.5 }] }, 0, "worker")).toBeUndefined();
  expect(foregroundRows(details(), true)[0].task).toBe(TASK_UNAVAILABLE);
});

test("slash task comes from exact initial request; composite placeholders fail closed", () => {
  const end = { type: "custom_message", customType: "subagent-slash-result", details: { requestId: "slash-id", result: { details: details() } } };
  expect([...restoreRecords([slashStart(), end], owner, false).children.values()][0].task).toBe("Slash task");
  const initial = slashStart(); initial.details.result.details.mode = "parallel";
  expect([...restoreRecords([initial, end], owner, false).children.values()][0].task).toBe(TASK_UNAVAILABLE);
  expect(restoreRecords([end], owner, false).children.size).toBe(0);
  expect(branchOrigins([slashStart("other")]).slash.has("slash-id")).toBe(false);
});

test("late slash replies and tool events after tree switch never create new-branch observations", async () => {
  const f = fixture([slashStart("old-slash"), callEntry()]);
  f.bus.emit("subagent:slash:started", { requestId: "old-slash" });
  f.handlers.get("tool_execution_start")!({ toolName: "subagent", toolCallId: "call-a", args: { agent: "worker", task: "Old" } }, f.ctx);
  const newBranch: any[] = [];
  f.setBranch(newBranch);
  f.bus.emit("subagent:slash:response", { requestId: "old-slash", result: { details: details() } });
  f.handlers.get("tool_execution_update")!({ toolName: "subagent", toolCallId: "call-a", partialResult: { details: details() } }, f.ctx);
  f.handlers.get("tool_execution_end")!({ toolName: "subagent", toolCallId: "call-a", result: { details: details() } }, f.ctx);
  await f.command();
  expect(newBranch).toEqual([]);
  expect(f.notifications.join("\n")).toContain("No addressable");
  // Even a package-persisted late terminal result lacks its originating call.
  expect(restoreRecords([toolEntry()], owner, false).children.size).toBe(0);
  // A fresh slash request on the new branch still works.
  newBranch.push(slashStart("new-slash"));
  f.bus.emit("subagent:slash:started", { requestId: "new-slash" });
  f.bus.emit("subagent:slash:response", { requestId: "new-slash", result: { details: details() } });
  expect([...restoreRecords(newBranch, owner, false).children.values()][0].task).toBe("Slash task");
});

test("new exact-owner observations restore in fork, including partial progress without final record", async () => {
  const f = fixture([]);
  const forkOwner = { sessionId: "fork", sessionFile: "/sessions/fork" };
  const copied: any[] = [callEntry(), toolEntry()];
  f.replace(forkOwner, copied, true);
  expect(restoreRecords(copied, forkOwner, true).children.size).toBe(0);
  f.handlers.get("tool_execution_start")!({ toolName: "subagent", toolCallId: "new-call", args: { agent: "worker", task: "Fork task" } }, f.ctx);
  f.handlers.get("tool_execution_update")!({ toolName: "subagent", toolCallId: "new-call", partialResult: { details: { ...details(), runId: "new-fork-run" } } }, f.ctx);
  const restored = restoreRecords(copied, forkOwner, true);
  expect([...restored.children.values()].map(child => child.runId)).toEqual(["new-fork-run"]);
  expect([...restored.children.values()][0].task).toBe("Fork task");
  expect(restoreRecords(copied, owner, true).children.size).toBe(0);
});

test("single native revival receipt establishes index zero, composite and conflicting replies do not", () => {
  expect(resumedRun({ details: { mode: "single", runId: "new", asyncId: "new", results: [] } })).toBe("new");
  for (const data of [
    { asyncId: "new" }, { mode: "chain", asyncId: "new", results: [] },
    { mode: "single", asyncId: "new", runId: "different", results: [] },
    { mode: "single", asyncId: "new", results: [result(0), result(1)] },
    { mode: "single", asyncId: "new", results: [], workflowKey: "nested" },
  ]) expect(resumedRun({ details: data })).toBeUndefined();
});

test("targeted async refresh observes only exact Hub roots, never launches or enumerates nested rows", async () => {
  const f = fixture([acceptedReceipt()]);
  await tick(); f.requests.length = 0;
  f.setResponse(r => f.respond(r, r.method === "ping" ? { version: 1, session: owner } : { text: "State: running", asyncSnapshot: { ...nativeSnapshot("hub-run", "running"), runs: [...nativeSnapshot("hub-run", "running").runs, ...nativeSnapshot("foreign", "complete").runs] } }));
  f.bus.emit("subagent:jobs-changed", { version: 1 }); await tick();
  expect(f.requests.map(r => [r.method, r.params])).toEqual([["ping", {}], ["status", { id: "hub-run", index: 0 }]]);
  expect(childFrom(f)?.status).toBe("running");
  expect(childFrom(f)?.model).toBe("openai/resolved");
  expect(restoreRecords(f.entries, owner, false).children.size).toBe(1);
  expect(f.requests.some(r => r.method === "resume")).toBe(false);
  const child = childFrom(f)!;
  expect(snapshotObservation(child, nativeSnapshot("unrelated", "complete", { children: [{ ...nativeSnapshot("hub-run", "complete").runs[0] }] }))).toBeUndefined();
  expect(snapshotObservation(child, nativeSnapshot("hub-run", "complete", { children: [{ id: "step:1", kind: "step" }] }))).toBeUndefined();
});

test("completion retains child identity and lineage, enables only an explicit follow-up from latest run", async () => {
  const f = fixture([acceptedReceipt()]); await tick();
  f.handlers.get("message_end")!({ message: { role: "custom", customType: "subagent-notify", details: nativeNotice() } }, f.ctx);
  expect(childFrom(f)?.status).toBe("completed");
  expect(childFrom(f)?.sessionFile).toBe("/sessions/revived.jsonl");
  expect(childFrom(f)?.previous).toEqual({ runId: "exact-run-123", index: 0 });
  f.setResponse(r => f.respond(r, r.method === "ping" ? { version: 1, session: owner, capabilities: { resume: true } } : r.method === "resume" ? { details: { mode: "single", runId: "next-hub-run", asyncId: "next-hub-run", results: [] } } : { text: "Completed" }));
  f.selections.push(0, 3, undefined); f.inputs.push("Second follow-up"); f.confirms.push(true);
  await f.command(); await tick();
  expect(f.requests.filter(r => r.method === "resume").map(r => r.params)).toEqual([{ id: "hub-run", index: 0, message: "Second follow-up" }]);
  const latest = childFrom(f, "next-hub-run")!;
  expect(latest.previous).toEqual({ runId: "hub-run", index: 0 });
  expect(latest.root).toEqual({ runId: "exact-run-123", index: 0 });
  expect(latest.status).toBe("accepted");
  expect(latest.sessionFile).toBeUndefined();
  expect(latest.task).toBe("Second follow-up");
});

test("native completion records restore only exact accepted owner targets, even before the receipt", () => {
  const notice = { type: "custom_message", customType: "subagent-notify", details: { runs: [...nativeNotice().runs, ...nativeNotice("foreign").runs] } };
  const state = restoreRecords([notice, acceptedReceipt()], owner, false);
  expect(state.children.size).toBe(1);
  expect([...state.children.values()][0].status).toBe("completed");
  expect(restoreRecords([notice, acceptedReceipt()], { ...owner, sessionId: "fork" }, true).children.size).toBe(0);
  expect(completionObservation([...state.children.values()][0], { runs: [{ ...nativeNotice().runs[0], workflowRunId: "workflow" }] })).toBeUndefined();
});

test("duplicate completion callbacks are idempotent and a stale running reply cannot regress completion", async () => {
  const f = fixture([acceptedReceipt()]); await tick();
  let pending: any;
  f.setResponse(r => { if (r.method === "status") pending = r; else f.respond(r, { version: 1, session: owner }); });
  f.bus.emit("subagent:jobs-changed", {}); await tick();
  const event = { message: { role: "custom", customType: "subagent-notify", details: nativeNotice() } };
  f.handlers.get("message_end")!(event, f.ctx);
  const size = f.entries.length;
  f.handlers.get("message_end")!(event, f.ctx);
  expect(f.entries.length).toBe(size);
  f.respond(pending, { asyncSnapshot: nativeSnapshot("hub-run", "running") }); await tick();
  expect(childFrom(f)?.status).toBe("completed");
  expect(f.entries.length).toBe(size);
});

test("refresh bursts coalesce and retain one invalidation arriving during a status request", async () => {
  const f = fixture([acceptedReceipt()]); await tick(); f.requests.length = 0;
  const pending: any[] = [];
  f.setResponse(r => { if (r.method === "status") pending.push(r); else f.respond(r, { version: 1, session: owner }); });
  f.bus.emit("subagent:jobs-changed", {}); await tick();
  for (let i = 0; i < 10; i++) f.bus.emit("subagent:jobs-changed", {});
  expect(pending.length).toBe(1);
  f.respond(pending[0], { asyncSnapshot: nativeSnapshot("hub-run", "running") }); await tick();
  expect(pending.length).toBe(2);
  f.respond(pending[1], { asyncSnapshot: nativeSnapshot("hub-run", "complete") }); await tick();
  expect(pending.length).toBe(2);
  expect(childFrom(f)?.status).toBe("completed");
});

test("unavailable refresh API warns once, preserves recorded rows, and never retries queued hints", async () => {
  const f = fixture([acceptedReceipt()]); await tick(); f.requests.length = 0;
  const before = structuredClone(f.entries);
  f.setResponse(() => {});
  const warned = new Promise<void>(resolve => {
    f.ctx.ui.notify = text => { f.notifications.push(text); resolve(); };
  });
  f.bus.emit("subagent:jobs-changed", {});
  for (let i = 0; i < 10; i++) f.bus.emit("subagent:jobs-changed", {});
  await warned; await tick();
  expect(f.notifications).toHaveLength(1);
  expect(f.notifications[0]).toContain("API unavailable");
  expect(f.notifications[0]).toContain("recorded rows may be stale");
  expect(f.entries).toEqual(before);
  expect(f.requests.map(r => r.method)).toEqual(["ping"]);
  expect([...f.bus.handlers.keys()].filter(name => name.startsWith(REPLY))).toEqual([]);
});

test.each(["rejected", "status error", "transport"])("%s refresh warns without spam; recovery rearms diagnostics and explicit reads remain available", async failure => {
  const f = fixture([acceptedReceipt()]); await tick(); f.requests.length = 0;
  const before = structuredClone(f.entries);
  let reason = "First failure\x1b[31m\x07" + "x".repeat(3000);
  const fail = (r: any) => {
    if (r.method === "ping") { f.respond(r, { version: 1, session: owner }); return; }
    if (failure === "transport") throw new Error(reason);
    if (failure === "status error") f.respond(r, { isError: true, text: reason, asyncSnapshot: nativeSnapshot("hub-run", "complete") });
    else f.bus.emit(REPLY + r.requestId, { version: 1, requestId: r.requestId, success: false, error: { code: "unavailable", message: reason } });
  };
  f.setResponse(fail);
  for (let i = 0; i < 3; i++) { f.bus.emit("subagent:jobs-changed", {}); await tick(); }
  expect(f.notifications).toHaveLength(1);
  expect(f.notifications[0]).toContain("First failure");
  expect(f.notifications[0]).toContain("Read package status");
  expect(f.notifications[0].length).toBeLessThan(800);
  expect(f.notifications[0]).not.toMatch(/[\x1b\x07]/);
  expect(f.entries).toEqual(before);
  // Reading the preserved record and explicitly retrying status needs no resume.
  f.setResponse(r => f.respond(r, r.method === "ping" ? { version: 1, session: owner } : { text: "Recovered status", asyncSnapshot: nativeSnapshot("hub-run", "running") }));
  f.selections.push(0, 0, 1, undefined, undefined); await f.command();
  expect(f.screens.join("\n")).toContain("Recorded status: accepted");
  expect(f.screens.join("\n")).toContain("Recovered status");
  f.bus.emit("subagent:jobs-changed", {}); await tick();
  expect(childFrom(f)?.status).toBe("running");
  reason = "Next distinct failure";
  f.setResponse(fail);
  f.bus.emit("subagent:async-complete", {}); await tick();
  f.bus.emit("subagents:rpc:v1:ready", {}); await tick();
  const warnings = f.notifications.filter(text => text.startsWith("Agent Hub refresh failed"));
  expect(warnings).toHaveLength(2);
  expect(warnings[1]).toContain(reason);
  expect(childFrom(f)?.status).toBe("running");
  expect(f.requests.every(r => ["ping", "status"].includes(r.method))).toBe(true);
});

test.each(["shutdown", "branch", "owner"])("pending refresh failure after %s cannot warn or write into the destination", async change => {
  const f = fixture([acceptedReceipt()]); await tick();
  let pending: any;
  f.setResponse(r => { if (r.method === "status") pending = r; else f.respond(r, { version: 1, session: owner }); });
  f.bus.emit("subagent:jobs-changed", {}); await tick();
  expect(pending).toBeDefined();
  const before = structuredClone(f.entries);
  const destination: any[] = [];
  if (change === "owner") f.replace({ sessionId: "new-parent", sessionFile: "/new-parent" }, destination);
  else if (change === "branch") f.setBranch(destination);
  else f.stop();
  f.bus.emit(REPLY + pending.requestId, { version: 1, requestId: pending.requestId, success: false, error: { code: "unavailable", message: "Old request failed" } });
  await tick();
  expect(f.notifications).toEqual([]);
  expect(f.entries).toEqual(before);
  expect(destination).toEqual([]);
  expect([...f.bus.handlers.keys()].filter(name => name.startsWith(REPLY))).toEqual([]);
});

test("late async status across branch or owner replacement cannot persist into destination", async () => {
  for (const replace of [false, true]) {
    const f = fixture([acceptedReceipt()]); await tick();
    let pending: any;
    f.setResponse(r => { if (r.method === "status") pending = r; else f.respond(r, { version: 1, session: owner }); });
    f.bus.emit("subagent:jobs-changed", {}); await tick();
    const destination: any[] = [];
    if (replace) f.replace({ sessionId: "new-parent", sessionFile: "/new-parent" }, destination);
    else f.setBranch(destination);
    f.respond(pending, { asyncSnapshot: nativeSnapshot("hub-run", "complete") }); await tick();
    expect(destination).toEqual([]);
    expect([...f.bus.handlers.keys()].filter(name => name.startsWith(REPLY))).toEqual([]);
  }
});

test("malformed resume is unknown, duplicate replies settle once, and tree history cannot redispatch", async () => {
  const f = fixture();
  const originalBranch = [...f.entries];
  f.setResponse(r => {
    if (r.method === "resume") {
      f.respond(r, { details: { mode: "chain", asyncId: "ambiguous", results: [] } });
      f.respond(r, { details: { mode: "single", asyncId: "duplicate", results: [] } });
    } else f.respond(r, r.method === "ping" ? { version: 1, session: owner, capabilities: { resume: true } } : { text: "Completed" });
  });
  f.selections.push(0, 3, undefined, undefined); f.inputs.push("Explicit"); f.confirms.push(true); await f.command();
  const state = restoreRecords(f.entries, owner, false);
  expect([...state.receipts.values()][0].state).toBe("unknown");
  expect(state.children.size).toBe(1);
  f.setBranch(originalBranch);
  f.selections.push(0, 3, undefined, undefined); await f.command();
  expect(f.requests.filter(r => r.method === "resume")).toHaveLength(1);
  expect(f.notifications.join("\n")).toContain("will not redispatch");
  expect(restoreRecords(originalBranch, owner, false).children.size).toBe(1);
});

test("concurrent Hub commands cannot duplicate a pending revival", async () => {
  const f = fixture(); let pending: any;
  f.setResponse(r => {
    if (r.method === "resume") pending = r;
    else f.respond(r, r.method === "ping" ? { version: 1, session: owner, capabilities: { resume: true } } : { text: "Completed" });
  });
  f.selections.push(0, 3, undefined); f.inputs.push("Explicit"); f.confirms.push(true);
  const first = f.command(); await tick();
  await f.command();
  expect(f.notifications.join("\n")).toContain("already open");
  expect(f.requests.filter(r => r.method === "resume")).toHaveLength(1);
  f.respond(pending, { details: { mode: "single", asyncId: "one-run", results: [] } });
  await first;
});

test("transport dispatch failure stays unknown and survives reload without retry", async () => {
  const f = fixture();
  f.setResponse(r => {
    if (r.method === "resume") throw new Error("transport disconnected");
    f.respond(r, r.method === "ping" ? { version: 1, session: owner, capabilities: { resume: true } } : { text: "Completed" });
  });
  f.selections.push(0, 3, undefined, undefined); f.inputs.push("Explicit"); f.confirms.push(true); await f.command();
  expect([...restoreRecords(f.entries, owner, false).receipts.values()][0].state).toBe("unknown");
  expect(f.notifications.join("\n")).toContain("execution may have started");
  f.stop(); f.handlers.get("session_start")!({}, f.ctx);
  f.selections.push(0, 3, undefined, undefined); await f.command();
  expect(f.requests.filter(r => r.method === "resume")).toHaveLength(1);
});

test.each(["paused", "failed", "complete"])("package stop evidence blocks stale confirmation despite delayed %s snapshot", async delayed => {
  const f = fixture([acceptedReceipt(), { type: "custom_message", customType: "subagent-notify", details: nativeNotice("hub-run", "paused") }]);
  await tick();
  // Paused can be sealed as stopped by package-native control after selection.
  f.ctx.ui.confirm = async () => {
    f.handlers.get("message_end")!({ message: { role: "custom", customType: "subagent-notify", details: nativeNotice("hub-run", "stopped") } }, f.ctx);
    f.setResponse(r => f.respond(r, r.method === "ping" ? { version: 1, session: owner, capabilities: { resume: true } } : { asyncSnapshot: nativeSnapshot("hub-run", delayed) }));
    f.bus.emit("subagent:jobs-changed", {});
    await tick();
    return true;
  };
  f.selections.push(0, 3, undefined, undefined); f.inputs.push("Explicit"); await f.command();
  expect(f.requests.filter(r => r.method === "resume")).toHaveLength(0);
  expect(childFrom(f)?.status).toBe("stopped");
});

test("branch return restores only that branch's receipt and terminal observations", async () => {
  const f = fixture([acceptedReceipt()]); await tick();
  f.handlers.get("message_end")!({ message: { role: "custom", customType: "subagent-notify", details: nativeNotice() } }, f.ctx);
  f.setBranch([]); await f.command();
  expect(f.notifications.join("\n")).toContain("No addressable");
  f.setBranch(f.entries); await tick();
  f.selections.push(0, 0, undefined, undefined); await f.command();
  expect(f.screens.join("\n")).toContain("Recorded status: completed");
  expect(f.screens.join("\n")).toContain("Previous: exact-run-123 / index 0");
  expect(f.requests.some(r => r.method === "resume")).toBe(false);
});

test("owner replacement during dispatched resume keeps only original unresolved receipt", async () => {
  const f = fixture(); let pending: any;
  f.setResponse(r => {
    if (r.method === "resume") pending = r;
    else f.respond(r, r.method === "ping" ? { version: 1, session: owner, capabilities: { resume: true } } : { text: "Completed" });
  });
  f.selections.push(0, 3); f.inputs.push("Explicit"); f.confirms.push(true);
  const action = f.command(); await tick();
  const destination: any[] = [];
  f.replace({ sessionId: "new-parent", sessionFile: "/new-parent" }, destination);
  f.respond(pending, { details: { mode: "single", asyncId: "late-run", results: [] } });
  await action;
  expect(destination).toEqual([]);
  expect([...restoreRecords(f.entries, owner, false).receipts.values()][0].state).toBe("requested");
});

test.each(["paused", "failed", "complete"])("stopped is irreversible across delayed %s snapshot and reload", delayed => {
  const receipt = acceptedReceipt();
  const initial = [...restoreRecords([receipt], owner, false).children.values()][0];
  const stopped = completionObservation(initial, nativeNotice("hub-run", "stopped"))!;
  const stale = snapshotObservation(initial, nativeSnapshot("hub-run", delayed))!;
  expect(mergeChild(stopped, stale)).toEqual(stopped);
  const observation = (child: typeof stopped) => ({ type: "custom", customType: OBSERVATION, data: { owner, runId: child.runId, child } });
  const restored = restoreRecords([receipt, observation(stopped), observation(stale)], owner, false);
  expect(restored.children.get(childKey(stopped))).toEqual(stopped);
  expect(mergeChild(stopped, { ...stale, evidence: "completion" })).toEqual(stopped);
  expect(mergeChild(stopped, { ...stale, runId: "different-run" }).status).toBe(stale.status);
});

test("native terminal evidence outranks terminal snapshots but paused can become stopped", () => {
  const initial = [...restoreRecords([acceptedReceipt()], owner, false).children.values()][0];
  for (const status of ["completed", "failed", "paused"]) {
    const notice = completionObservation(initial, nativeNotice("hub-run", status))!;
    expect(mergeChild(notice, snapshotObservation(initial, nativeSnapshot("hub-run", "failed"))!)).toEqual(notice);
    expect(mergeChild(notice, snapshotObservation(initial, nativeSnapshot("hub-run", "stopped"))!).status).toBe("stopped");
    expect(mergeChild(notice, completionObservation(initial, nativeNotice("hub-run", "stopped"))!).status).toBe("stopped");
  }
});

const workerReport = (status = "completed") => ({ contract: "pi-worker-completion-v1", status, checks: [{ command: "node --test", result: "passed", evidence: "2 tests passed" }], incomplete: status === "completed" ? [] : ["Required integration check pending"] });
// SingleResult.acceptance is the package ledger, not structuredOutput or childReport.
const hostLedger = () => ({ status: "rejected", evidenceStatus: "rejected", explicit: true, effectiveAcceptance: { level: "verified" }, inferredReason: [], criteria: [], childReport: { commandsRun: [{ command: "pretend independent check", result: "passed" }] }, runtimeChecks: [{ id: "no-staged-files", status: "passed", message: "Index empty" }], verifyRuns: [{ id: "focused", command: "npm test", status: "failed", exitCode: 1, durationMs: 3, stdout: "1 failing test" }] });

test.each(["completed", "blocked", "failed"])("worker %s verdict is separate from successful execution and rejected host evidence", status => {
  const native = details([{ ...result(), structuredOutput: workerReport(status), acceptance: hostLedger() }] as any);
  const row = foregroundRows(native, true)[0];
  expect(row.status).toBe("completed");
  expect(row.completion?.status).toBe(status);
  expect(row.acceptance?.status).toBe("rejected");
  expect(row.acceptance?.verifyRuns).toEqual([{ command: "npm test", status: "failed", exitCode: 1 }]);
  const text = completionText(row);
  expect(text).toContain(`${status} (self-reported)`);
  expect(text).toContain("Host verification commands");
  expect(text).not.toContain("pretend independent check");
  expect(text).toContain("Hub runs no verifier");
  expect(foregroundRows(native, false)[0].completion).toBeUndefined();
  expect(foregroundRows(native, false)[0].acceptance).toBeUndefined();
  expect(restoreRecords([callEntry(), toolEntry(native)], owner, false).children.get(childKey(row))?.completion).toEqual(row.completion);
});

test("prose, missing/invalid contract and package structured-output failure never establish a verdict", () => {
  for (const extra of [
    { finalOutput: JSON.stringify(workerReport()) },
    { structuredOutput: { status: "completed" } },
    { structuredOutput: { ...workerReport(), incomplete: ["Required work unfinished"] } },
    { structuredOutput: workerReport(), structuredOutputFailed: true },
    { structuredOutput: { ...workerReport(), checks: Array(17).fill(workerReport().checks[0]) } },
  ]) {
    const row = foregroundRows(details([{ ...result(), ...extra }] as any), true)[0];
    expect(row.completion).toBeUndefined();
    expect(completionText(row)).toContain("Worker verdict: unknown");
  }
  expect(acceptanceEvidence({ ...hostLedger(), verifyRuns: [{ command: "npm test", status: "passed" }] })).toBeUndefined();
  expect(acceptanceEvidence({ childReport: hostLedger().childReport })).toBeUndefined();
  expect(acceptanceEvidence({ ...hostLedger(), verifyRuns: [{ ...hostLedger().verifyRuns[0], status: "passed", exitCode: 0, memoized: true }] })?.verifyRuns[0].memoized).toBe(true);
  expect(acceptanceEvidence({ ...hostLedger(), verifyRuns: Array(17).fill(hostLedger().verifyRuns[0]) })).toBeUndefined();
});

test("exact-owner observation restores validated verdict; malformed retained output stays unknown", () => {
  const child = foregroundRows(details([{ ...result(), structuredOutput: workerReport() }] as any), true)[0];
  const observation = (value: unknown) => ({ type: "custom", customType: OBSERVATION, data: { owner, runId: child.runId, child: value } });
  expect(restoreRecords([observation(child)], owner, true).children.get(childKey(child))?.completion).toEqual(child.completion);
  const invalid = restoreRecords([observation({ ...child, completion: { ...workerReport(), incomplete: ["pending"] } })], owner, true).children.get(childKey(child))!;
  expect(invalid.completion).toBeUndefined();
  expect(invalid.completionInvalid).toBe(true);
});

test("new Hub receipt never inherits source verdict or host checks; aggregate completion leaves verdict unknown", () => {
  const target = foregroundRows(details([{ ...result(), structuredOutput: workerReport(), acceptance: hostLedger() }] as any), true)[0];
  const receipt = acceptedReceipt("hub-run", target);
  const child = [...restoreRecords([receipt], owner, false).children.values()][0];
  expect(child.status).toBe("accepted");
  expect(child.completion).toBeUndefined();
  expect(child.acceptance).toBeUndefined();
  const observed = snapshotObservation(child, nativeSnapshot("hub-run", "complete"))!;
  const notice = completionObservation(observed, { runs: [{ ...nativeNotice().runs[0], structuredOutput: workerReport() }] })!;
  expect(notice.status).toBe("completed");
  expect(notice.completion).toBeUndefined();
  expect(completionText(notice)).toContain("Worker verdict: unknown");
  const restored = restoreRecords([receipt, { type: "custom_message", customType: "subagent-notify", details: nativeNotice() }], owner, false);
  expect([...restored.children.values()][0].completion).toBeUndefined();
});

test("Hub selector labels worker verdict separately and inspection presents self-report, without commands", async () => {
  const native = details([{ ...result(), structuredOutput: workerReport("blocked") }] as any);
  const f = fixture([callEntry(), toolEntry(native)]);
  const labels: string[] = [];
  const select = f.ctx.ui.select;
  f.ctx.ui.select = async (title, options) => { labels.push(...options); return select(title, options); };
  f.selections.push(0, 0, undefined, undefined);
  await f.command();
  expect(labels.join("\n")).toContain("execution: completed");
  expect(labels.join("\n")).toContain("worker verdict: blocked");
  expect(f.screens.join("\n")).toContain("blocked (self-reported)");
  expect(f.requests).toEqual([]);
});

test("late terminal-looking foreground progress cannot erase final verdict or host evidence", () => {
  const final = foregroundRows(details([{ ...result(), structuredOutput: workerReport(), acceptance: hostLedger() }] as any), true)[0];
  const progress = foregroundRows(details([{ ...result(), progress: { status: "completed" } }] as any), false)[0];
  expect(progress.completion).toBeUndefined();
  expect(mergeChild(final, progress)).toEqual(final);
  const observation = (child: typeof final) => ({ type: "custom", customType: OBSERVATION, data: { owner, runId: child.runId, child } });
  expect(restoreRecords([observation(final), observation(progress)], owner, false).children.get(childKey(final))).toEqual(final);
});
