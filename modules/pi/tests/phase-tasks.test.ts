import { describe, expect, mock, test } from "bun:test";
import { SessionManager, type ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { KeybindingsManager, TUI_KEYBINDINGS, visibleWidth } from "@earendil-works/pi-tui";
import phaseTasks from "../files/extensions/phase-tasks";
import {
  MAX_TASKS, SNAPSHOT_TYPE, allTasks, cloneState, decodeState, emptyState,
  phaseStatus, transition, treeText, type TaskInput, type TaskState,
} from "../files/extensions/phase-tasks/state";
import { createTaskViewer, widgetLine } from "../files/extensions/phase-tasks/view";

const plan: TaskInput = { action: "init", phases: [
  { name: "Investigate", tasks: ["Read code", "Check assumptions"] },
  { name: "Implement", tasks: ["Patch", "Verify"] },
] };
const init = () => transition(emptyState(), plan);
const status = (state: TaskState) => allTasks(state).map(t => t.status);
const plainTheme = { fg: (_: string, text: string) => text };
const keys = new KeybindingsManager(TUI_KEYBINDINGS);
type CapturedTool = Parameters<ExtensionAPI["registerTool"]>[0];
type CapturedCommand = Parameters<ExtensionAPI["registerCommand"]>[1];
type WidgetFactory = (tui: object, theme: typeof plainTheme) => { render(width: number): string[] };
type ViewerFactory = (tui: { terminal: { rows: number }; requestRender(): void }, theme: typeof plainTheme, keys: KeybindingsManager, done: () => void) => { dispose(): void };

function harness(sm = SessionManager.inMemory("/tmp/phase-tasks-test"), mode = "json") {
  const handlers = new Map<string, Function>();
  let tool!: CapturedTool;
  let command!: CapturedCommand;
  const ui = {
    setWidget: mock((_name: string, _factory?: WidgetFactory | string[]) => {}),
    notify: mock(() => {}),
    custom: mock(async (_factory: ViewerFactory) => {}),
  };
  const append = mock((type: string, data: unknown) => { sm.appendCustomEntry(type, data); });
  const ctx = { mode, hasUI: mode === "tui" || mode === "rpc", ui, sessionManager: sm };
  phaseTasks({
    on: (event: string, handler: Function) => handlers.set(event, handler),
    registerTool: (value: CapturedTool) => { tool = value; },
    registerCommand: (_name: string, value: CapturedCommand) => { command = value; },
    appendEntry: append,
  } as never);
  const event = (name: string, reason?: string) => handlers.get(name)!({ reason }, ctx);
  event("session_start", "startup");
  const call = (params: TaskInput, signal?: AbortSignal) => tool.execute("call", params, signal, undefined, ctx);
  const view = async (): Promise<TaskState> => (await call({ action: "view" })).details.snapshot;
  return { sm, ctx, ui, append, tool, command, event, call, view };
}

describe("phase task transactions", () => {
  test("explicit start, single active task, and ordered promotion across phases", () => {
    const pending = init();
    expect(status(pending)).toEqual(["pending", "pending", "pending", "pending"]);
    let state = transition(pending, { action: "start" });
    expect(status(state)).toEqual(["in_progress", "pending", "pending", "pending"]);
    expect(() => transition(state, { action: "start", target: "Patch" })).toThrow("current task");
    state = transition(state, { action: "done" });
    expect(status(state)).toEqual(["completed", "in_progress", "pending", "pending"]);
    state = transition(state, { action: "done", target: "Check assumptions" });
    expect(phaseStatus(state.phases[0])).toBe("completed");
    expect(status(state)).toEqual(["completed", "completed", "in_progress", "pending"]);
    state = transition(state, { action: "drop", target: "Patch" });
    expect(status(state)).toEqual(["completed", "completed", "abandoned", "in_progress"]);
    state = transition(state, { action: "done" });
    expect(phaseStatus(state.phases[1])).toBe("abandoned");
    expect(widgetLine(state, 80)).toBeUndefined();
    expect(status(pending)).toEqual(["pending", "pending", "pending", "pending"]);
  });

  test("pending drop/remove never starts execution, active remove promotes and prunes empty phases", () => {
    let state = transition(init(), { action: "drop", target: "Read code" });
    expect(status(state)).toEqual(["abandoned", "pending", "pending", "pending"]);
    state = transition(state, { action: "remove", target: "Read code" });
    state = transition(state, { action: "start" });
    state = transition(state, { action: "remove", target: "Check assumptions" });
    expect(state.phases.map(p => p.name)).toEqual(["Implement"]);
    expect(status(state)).toEqual(["in_progress", "pending"]);
    state = transition(state, { action: "remove", target: "Patch" });
    state = transition(state, { action: "remove", target: "Verify" });
    expect(state.phases).toEqual([]);
    expect(widgetLine(state, 80)).toBeUndefined();
  });

  test("append is ordered, does not preempt active work, and never reuses removed/replaced IDs", () => {
    let state = transition(init(), { action: "start" });
    const oldIds = allTasks(state).map(t => t.id);
    state = transition(state, { action: "append", phase: "Investigate", tasks: ["Extra check"] });
    state = transition(state, { action: "append", phases: [{ name: "Ship", tasks: ["Report"] }] });
    expect(allTasks(state).map(t => t.name)).toEqual(["Read code", "Check assumptions", "Extra check", "Patch", "Verify", "Report"]);
    expect(allTasks(state).filter(t => t.status === "in_progress")).toHaveLength(1);
    state = transition(state, { action: "remove", target: "Extra check" });
    state = transition(state, { action: "init", replace: true, phases: [{ name: "New plan", tasks: ["New task"] }] });
    expect(oldIds).not.toContain(state.phases[0].tasks[0].id);
    expect(Number(state.phases[0].tasks[0].id.slice(1))).toBeGreaterThan(10);
    expect(status(state)).toEqual(["pending"]);
  });

  test("duplicate exact names require phase scope or stable IDs, and IDs take precedence", () => {
    let state = transition(emptyState(), { action: "init", phases: [
      { name: "First", tasks: ["Same", "t2"] }, { name: "Second", tasks: ["Same"] },
    ] });
    expect(() => transition(state, { action: "start", target: "Same" })).toThrow("ambiguous");
    state = transition(state, { action: "start", phase: "Second", target: "Same" });
    expect(status(state)).toEqual(["pending", "pending", "in_progress"]);
    state = transition(state, { action: "done" });
    expect(status(state)).toEqual(["in_progress", "pending", "completed"]);
    state = transition(state, { action: "done", target: "t2" });
    expect(status(state)).toEqual(["completed", "in_progress", "completed"]);
  });

  test.each([
    { action: "init", phases: [{ name: "Replacement", tasks: ["No consent"] }] },
    { action: "done", target: "Patch" },
    { action: "drop" },
    { action: "remove", target: "missing" },
    { action: "append", phase: "Investigate", tasks: ["Valid", "  "] },
    { action: "append", phases: [{ name: "Valid", tasks: ["One"] }, { name: "Invalid", tasks: [] }] },
    { action: "append", phase: "Investigate", tasks: ["One"], phases: [{ name: "Other", tasks: ["Two"] }] },
    { action: "append", phase: "Investigate", tasks: ["\x1b]52;c;clipboard\x07"] },
    { action: "append", phase: "Investigate", tasks: Array(MAX_TASKS).fill("Overflow") },
    { action: "view", target: "ignored?" },
  ] as TaskInput[])("rejects invalid mutation atomically: %j", input => {
    const state = transition(init(), { action: "start" });
    const original = cloneState(state);
    expect(() => transition(state, input)).toThrow();
    expect(state).toEqual(original);
  });

  test("bounds UTF-8 snapshots and rejects invalid persisted invariants", () => {
    expect(() => transition(emptyState(), { action: "init", phases: [{ name: "Large", tasks: Array(100).fill("한".repeat(160)) }] })).toThrow("bytes");
    const state = init();
    state.phases[0].tasks[0].status = "in_progress";
    state.phases[1].tasks[0].status = "in_progress";
    expect(() => decodeState(state)).toThrow("only one");
    state.phases[1].tasks[0].status = "pending";
    state.nextId = 1;
    expect(() => decodeState(state)).toThrow("id");
  });
});

describe("session-local persistence and lifecycle", () => {
  test("restores only the current branch after tree navigation, resume, fork and reload", async () => {
    const h = harness();
    await h.call(plan);
    const pendingLeaf = h.sm.getLeafId()!;
    await h.call({ action: "start" });
    const startedLeaf = h.sm.getLeafId()!;
    await h.call({ action: "done" });
    const completedLeaf = h.sm.getLeafId()!;
    h.sm.branch(pendingLeaf);
    h.event("session_tree");
    expect(status(await h.view())).toEqual(["pending", "pending", "pending", "pending"]);
    await h.call({ action: "drop", target: "Patch" });
    const alternateLeaf = h.sm.getLeafId()!;
    h.sm.branch(completedLeaf);
    h.event("session_tree");
    expect(status(await h.view())).toEqual(["completed", "in_progress", "pending", "pending"]);
    h.event("session_shutdown", "reload");
    await expect(h.call({ action: "view" })).rejects.toThrow("not active");
    const resumed = harness(h.sm);
    expect(status(await resumed.view())).toEqual(["completed", "in_progress", "pending", "pending"]);
    // A fork retains only the selected ancestry in a different session/runtime.
    h.sm.branch(startedLeaf);
    const fork = SessionManager.inMemory("/tmp/phase-tasks-fork");
    for (const entry of h.sm.getBranch()) {
      if (entry.type === "custom") fork.appendCustomEntry(entry.customType, entry.data);
    }
    const child = harness(fork);
    expect(status(await child.view())).toEqual(["in_progress", "pending", "pending", "pending"]);
    await child.call({ action: "drop", target: "Read code" });
    h.sm.branch(alternateLeaf);
    resumed.event("session_start", "resume");
    expect(status(await resumed.view())).toEqual(["pending", "pending", "abandoned", "pending"]);
    expect((await harness().view()).phases).toEqual([]);
    h.sm.resetLeaf();
    resumed.event("session_tree");
    expect((await resumed.view()).phases).toEqual([]);
  });

  test("failed validation, cancellation and persistence errors leave state and history untouched", async () => {
    const h = harness();
    await h.call(plan);
    const before = await h.view();
    const leaf = h.sm.getLeafId();
    await expect(h.call({ action: "append", phase: "Investigate", tasks: ["Good", ""] })).rejects.toThrow();
    const cancelled = new AbortController();
    cancelled.abort();
    await expect(h.call({ action: "start" }, cancelled.signal)).rejects.toThrow();
    h.append.mockImplementationOnce(() => { throw new Error("disk unavailable"); });
    await expect(h.call({ action: "start" })).rejects.toThrow("disk unavailable");
    expect(await h.view()).toEqual(before);
    expect(h.sm.getLeafId()).toBe(leaf);
    await h.call({ action: "start" });
    expect(status(await h.view())[0]).toBe("in_progress");
  });

  test("snapshots are detached and authoritative over late/out-of-order tool-result details", async () => {
    const h = harness();
    const original = await h.call(plan);
    const [first] = await Promise.all([
      h.call({ action: "append", phase: "Investigate", tasks: ["First append"] }),
      h.call({ action: "append", phase: "Investigate", tasks: ["Second append"] }),
    ]);
    original.details.snapshot.phases[0].tasks[0].name = "tampered result";
    h.sm.appendMessage({ role: "toolResult", toolCallId: "late", toolName: "task_list", content: [], details: first.details, isError: false, timestamp: 1 });
    h.event("session_start", "reload");
    const state = await h.view();
    expect(allTasks(state)).toHaveLength(6);
    expect(state.phases[0].tasks[0].name).toBe("Read code");
    expect(allTasks(state).map(t => t.name)).toContain("Second append");
  });

  test("corrupt latest snapshot fails closed rather than resurrecting an older plan", async () => {
    const h = harness();
    await h.call(plan);
    h.sm.appendCustomEntry(SNAPSHOT_TYPE, { version: 1, nextId: 0, phases: [] });
    h.event("session_tree");
    await expect(h.view()).rejects.toThrow("Cannot restore");
    await expect(h.call({ action: "start" })).rejects.toThrow("Cannot restore");
    await h.call({ ...plan, replace: true });
    expect(status(await h.view())).toEqual(["pending", "pending", "pending", "pending"]);
  });

  test("headless view returns full tree without UI; RPC /tasks uses notification", async () => {
    const h = harness();
    await h.call(plan);
    expect((await h.call({ action: "view" })).content[0].text).toContain("Implement [pending]");
    await h.command.handler("", h.ctx);
    expect(h.ui.custom).not.toHaveBeenCalled();
    expect(h.ui.setWidget).not.toHaveBeenCalled();
    const rpc = harness(h.sm, "rpc");
    await rpc.command.handler("", rpc.ctx);
    expect(rpc.ui.notify).toHaveBeenCalledWith(treeText(await rpc.view()), "info");
    expect(rpc.ui.custom).not.toHaveBeenCalled();
  });

  test("TUI widget hides on completion and shutdown; tree changes close an open viewer", async () => {
    const h = harness(undefined, "tui");
    await h.call({ action: "init", phases: [{ name: "Small", tasks: ["Only task"] }] });
    const factory = h.ui.setWidget.mock.calls.at(-1)?.[1];
    if (typeof factory !== "function") throw new Error("Expected a task widget factory");
    const widget = factory({}, plainTheme);
    expect(widget.render(80).join(" ")).toContain("Small / Only task");
    expect(widget.render(80).join(" ")).toContain("1 left");
    let closed = 0;
    let dispose = () => {};
    h.ui.custom.mockImplementationOnce((factory: ViewerFactory) => {
      const { promise, resolve } = Promise.withResolvers<void>();
      const component = factory({ terminal: { rows: 20 }, requestRender() {} }, plainTheme, keys, () => { closed++; resolve(); });
      dispose = component.dispose;
      return promise;
    });
    const command = h.command.handler("", h.ctx);
    h.event("session_tree");
    await command;
    dispose();
    expect(closed).toBe(1);
    await h.call({ action: "start" });
    await h.call({ action: "done" });
    expect(h.ui.setWidget).toHaveBeenLastCalledWith("phase-tasks", undefined);
    h.event("session_shutdown", "new");
    expect(h.ui.setWidget).toHaveBeenLastCalledWith("phase-tasks", undefined);
  });
});

test("native viewer wraps and scrolls the entire tree at narrow widths", () => {
  const state = transition(emptyState(), { action: "init", phases: [{ name: "한글 Phase", tasks: Array.from({ length: 60 }, (_, i) => `Task ${i} with a long name`) }] });
  let closed = 0;
  const viewer = createTaskViewer(state, plainTheme as never, keys, () => 16, () => {}, () => closed++);
  for (const width of [1, 2, 5, 20, 80]) {
    const lines = viewer.render(width);
    expect(lines.length).toBeLessThanOrEqual(14);
    for (const line of lines) expect(visibleWidth(line)).toBeLessThanOrEqual(width);
    expect(visibleWidth(widgetLine(state, width)!)).toBeLessThanOrEqual(width);
  }
  const before = viewer.render(80).join("\n");
  viewer.handleInput!("\x1b[6~");
  expect(viewer.render(80).join("\n")).not.toBe(before);
  for (let i = 0; i < 100; i++) viewer.handleInput!("\x1b[6~");
  expect(viewer.render(80).join("\n")).toContain("Task 59");
  viewer.invalidate();
  viewer.handleInput!("\x1b");
  expect(closed).toBe(1);
  expect(viewer.render(0)).toEqual([]);
});
