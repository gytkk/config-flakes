// Run against the unmodified npm package in a disposable Node process.
import assert from "node:assert/strict";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";
import { createRequire } from "node:module";
const [piEntry, cwd] = process.argv.slice(2);
const pi = await import(pathToFileURL(piEntry));
const require = createRequire(piEntry);
const { visibleWidth, stripTerminalSequences } = await import(pathToFileURL(require.resolve("@earendil-works/pi-tui")));
pi.initTheme("light", false);
const { theme } = await import(pathToFileURL(join(dirname(piEntry), "modes/interactive/theme/theme.js")));
globalThis.fetch = async () => { throw new Error("No network calls in the todo fixture"); };
const widgets = new Map(), notices = [];
const tui = { requestRender() {}, terminal: { columns: 80, rows: 24 } };
const ui = {
  theme, getToolsExpanded: () => false,
  notify: (message, level) => notices.push({ message, level }),
  setWidget(key, factory, options) {
    if (factory) widgets.set(key, { component: factory(tui, theme), options });
    else widgets.delete(key);
  },
};
const manager = pi.SessionManager.inMemory(cwd);
manager.appendCustomEntry("phase-tasks.snapshot.v1", { version: 1, nextId: 2, phases: [] });
const originalEntry = structuredClone(manager.getBranch()[0]);
const ctx = { hasUI: true, mode: "tui", cwd, sessionManager: manager, ui };
let loaded, extension, definition, appendTarget = manager;
async function emit(type, context = ctx, extra = {}) {
  for (const handler of extension.handlers.get(type) ?? []) await handler({ type, ...extra }, context);
}
async function load() {
  loaded = await pi.discoverAndLoadExtensions([], cwd, process.env.PI_CODING_AGENT_DIR);
  assert.deepEqual(loaded.errors, []); assert.deepEqual(loaded.warnings ?? [], []);
  assert.equal(loaded.extensions.length, 1);
  extension = loaded.extensions[0];
  assert.deepEqual([...extension.tools.keys()], ["todo"]);
  assert.deepEqual([...extension.commands.keys()], ["todos"]);
  loaded.runtime.appendEntry = (type, data) => appendTarget.appendCustomEntry(type, data);
  definition = extension.tools.get("todo").definition;
  await emit("session_start");
}
async function call(args, context = ctx) {
  appendTarget = context.sessionManager;
  const result = await definition.execute("fixture", args, undefined, undefined, context);
  await emit("tool_execution_end", context, { toolName: "todo", isError: !!result.details.error });
  return result.details;
}
const text = () => widgets.get("rpiv-todos").component.render(80).map(stripTerminalSequences).join("\n");
try {
  await load();
  assert.equal((await call({ action: "list" })).tasks.length, 0, "legacy phase state is not converted");
  await extension.commands.get("todos").handler("", ctx);
  assert.match(notices.at(-1).message, /No todos/);
  const created = await call({ action: "create", subject: "한글 첫 작업", owner: "parent" });
  const firstId = created.tasks[0].id;
  const firstLeaf = manager.getLeafId();
  const second = await call({ action: "create", subject: "Dependent task", blockedBy: [firstId] });
  const secondId = second.tasks[1].id;
  assert.equal(widgets.get("rpiv-todos").options.placement, "aboveEditor");
  assert.match(text(), /한글 첫 작업/);
  for (const width of [5, 12, 40, 80]) assert(widgets.get("rpiv-todos").component.render(width).every(line => visibleWidth(line) <= width));
  const shortcut = extension.shortcuts.get("ctrl+shift+t");
  assert(shortcut);
  await shortcut.handler(ctx); assert(!text().includes("Dependent task"));
  await shortcut.handler(ctx); assert.match(text(), /Dependent task/);
  const snapshot = JSON.stringify(second.tasks), leaf = manager.getLeafId();
  const cycle = await call({ action: "update", id: firstId, addBlockedBy: [secondId] });
  assert.match(cycle.error, /cycle/);
  assert.equal(JSON.stringify(cycle.tasks), snapshot);
  assert.equal(manager.getLeafId(), leaf, "rejected mutation is not persisted");
  await call({ action: "update", id: firstId, status: "in_progress", activeForm: "첫 작업 진행" });
  const completed = await call({ action: "update", id: firstId, status: "completed" });
  assert.equal(completed.tasks[1].status, "pending", "completion does not auto-start the next task");
  text(); await emit("agent_start");
  assert(!text().includes("한글 첫 작업"));
  assert.equal((await call({ action: "list" })).tasks[0].status, "completed", "hidden rows stay in state");
  await extension.commands.get("todos").handler("", ctx);
  assert.match(notices.at(-1).message, /한글 첫 작업/);
  const completedLeaf = manager.getLeafId();
  manager.branch(firstLeaf); await emit("session_tree");
  assert.equal((await call({ action: "list" })).tasks.length, 1);
  manager.branch(completedLeaf); await emit("session_tree");
  await emit("session_compact");
  assert.equal((await call({ action: "list" })).tasks.length, 2);
  const child = { ...ctx, hasUI: false, mode: "json", sessionManager: pi.SessionManager.inMemory(cwd) };
  await emit("session_start", child);
  assert.equal((await call({ action: "list" }, child)).tasks.length, 0);
  await call({ action: "create", subject: "Child only" }, child);
  assert(!text().includes("Child only"));
  await emit("session_shutdown", child);
  assert.equal((await call({ action: "list" })).tasks.length, 2);
  await emit("session_shutdown"); assert.equal(widgets.size, 0);
  await load();
  assert.equal((await call({ action: "list" })).tasks.length, 2);
  assert.deepEqual(manager.getBranch()[0], originalEntry, "legacy session entry preserved");
  const cleared = await call({ action: "clear" });
  assert.equal(cleared.tasks.length, 0); assert.equal(widgets.size, 0);
  console.log("PASS todo: single registration, mutations/dependencies, branch and reload replay, session isolation, widget/collapse, legacy preservation");
} finally {
  if (extension) await emit("session_shutdown");
}
