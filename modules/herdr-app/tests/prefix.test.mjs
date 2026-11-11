import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { setImmediate } from "node:timers/promises";
import vm from "node:vm";

const source = await readFile(new URL("../files/mods/tui-prefix/main.js", import.meta.url), "utf8");

async function fixture(mode, activate = true, result = { code: 0, stdout: "/work/one\n/work/two\n", stderr: "" }, { machine = "remote" } = {}) {
  const context = vm.createContext({});
  const state = { closed: false, stopped: 0, focused: false, commands: [], tasks: [], input: {}, result, executions: [], overlays: [] };
  const input = { value: () => state.query, focus() {}, on(event, handler) { state.input[event] = handler; } };
  const exports = {
    "gpui-kit": { View: class {}, div() {} },
    "gpui-base": { v_flex() {}, h_flex() {}, InputState: { new: () => input }, Input: {} },
    herdr: {
      agents: () => [],
      spaces: () => [{ id: "space", name: "Work", machine }],
      panes: () => [],
      focused: () => ({ space: "space", tab: "tab", pane: "pane", terminal: machine ? `${machine}/t1` : "t1" }),
      theme: () => ({}),
      props: () => mode ? { overlay: "picker", mode } : null,
      closeOverlay() { state.closed = true; },
      openOverlay(options) { state.overlays.push(options); },
      nextCommand: () => new Promise(resolve => { state.deliver = resolve; }),
      run(command, args) {
        state.commands.push({ command, args });
        if (command === "pane.focus") state.focused = false;
      },
      exec: async (...args) => {
        state.executions.push(args);
        if (state.result instanceof Error) throw state.result;
        return state.result;
      },
      hc: () => { throw new Error("Prefix cancellation must not send terminal input"); },
    },
  };
  const module = new vm.SourceTextModule(source, { context });
  await module.link(name => new vm.SyntheticModule(Object.keys(exports[name]), function () {
    for (const [key, value] of Object.entries(exports[name])) this.setExport(key, value);
  }, { context }));
  await module.evaluate();
  const view = new module.namespace.default();
  const cx = {
    focus_handle: () => ({ focus() { state.focused = true; }, is_focused: () => state.focused }), notify() {},
    stop_propagation() { state.stopped++; },
    timer: { every(_ms, handler) {
      state.watchFocus = () => handler(cx);
      return { cancel() { state.watchCancelled = true; } };
    } },
    spawn(fn) { state.tasks.push(fn(cx)); },
  };
  view.init({}, cx);
  if (!mode) {
    state.tasks = []; // The subscription intentionally lives for the layout's lifetime.
    if (activate) {
      state.deliver("activate");
      await setImmediate();
    }
  }
  const key = (value, modifiers = {}, held = false) => view.key({
    key: value,
    keystroke: modifiers.control ? `ctrl-${value}` : value,
    modifiers, is_held: held,
  }, cx);
  return { view, state, cx, key };
}

test("the persistent status bar does not steal focus until activated and Escape restores it", async () => {
  const { key, view, state } = await fixture(undefined, false);
  assert.equal(view.mode, "idle");
  assert.equal(state.focused, false);
  state.deliver("activate");
  await setImmediate();
  assert.equal(view.mode, "prefix");
  assert.equal(state.focused, true);
  key("escape");
  assert.equal(view.mode, "idle");
  assert.equal(state.focused, false);
});

test("unknown and modified keys cancel without leaking input or triggering a plain-key command", async () => {
  for (const [keyName, modifiers] of [["u", {}], ["X", { control: true }]]) {
    const { key, view, state } = await fixture();
    key(keyName, modifiers);
    assert.equal(state.stopped, 1);
    assert.equal(view.mode, "idle");
    assert.equal(state.focused, false);
    assert.equal(state.commands.filter(c => c.command !== "pane.focus").length, 0);
  }
});

test("holding the prefix cannot send repeated control input", async () => {
  const { key, view, state } = await fixture();
  key("space", { control: true }, true);
  assert.equal(view.mode, "prefix");
  assert.equal(state.tasks.length, 0);
});

test("pressing the prefix again cancels and restores focus without sending terminal input", async () => {
  const { key, view, state } = await fixture();
  key("space", { control: true });
  await Promise.all(state.tasks);
  assert.equal(view.mode, "idle");
  assert.equal(state.focused, false);
  assert.equal(state.watchCancelled, true);
  assert.equal(state.tasks.length, 0);
});

test("the command-routed prefix also toggles off instead of reactivating", async () => {
  const { view, state } = await fixture();
  state.deliver("activate");
  await setImmediate();
  assert.equal(view.mode, "idle");
  assert.equal(state.focused, false);
  assert.equal(state.watchCancelled, true);
});

test("directory search preserves selection boundaries, wraps and handles an empty result", async () => {
  const { key, view, state, cx } = await fixture("directories");
  await Promise.all(state.tasks);
  key("up");
  assert.equal(view.at, 1);
  key("down");
  assert.equal(view.at, 0);
  state.query = "work two";
  state.input.change({}, cx);
  assert.equal(view.rows()[0].label, "/work/two");
  state.query = "missing";
  state.input.change({}, cx);
  key("down");
  state.input.submit();
  assert.equal(view.at, 0);
  assert.equal(state.closed, false);
  assert.equal(state.commands.length, 0);
});

test("a zoxide failure is distinguishable from an empty search and cannot create a space", async () => {
  const { view, state } = await fixture("directories", false, { code: 1, stdout: "", stderr: "database unavailable" });
  await Promise.all(state.tasks);
  assert.match(view.error, /database unavailable/);
  assert.equal(view.busy, false);
  state.input.submit();
  assert.equal(state.closed, false);
  assert.equal(state.commands.length, 0);
});

test("moving focus away clears prefix state without stealing the keyboard back", async () => {
  const { view, state } = await fixture();
  state.focused = false;
  state.watchFocus();
  assert.equal(view.mode, "idle");
  assert.equal(state.watchCancelled, true);
  assert.equal(state.commands.length, 0);
});

test("prefix+N preserves the WSL machine and literal Linux directory when creating a space", async () => {
  const environment = { machine: "wsl" };
  const prefix = await fixture(undefined, true, undefined, environment);
  prefix.key("n", { shift: true });
  assert.equal(prefix.view.mode, "idle");
  assert.equal(prefix.state.overlays[0].props.mode, "directories");

  const folder = "/home/user/한글 project's $draft; notes";
  const { view, state, cx } = await fixture("directories", true,
    { code: 0, stdout: `/work/other\r\n${folder}\r\n`, stderr: "" }, environment);
  await Promise.all(state.tasks);
  assert.deepEqual(JSON.parse(JSON.stringify(state.executions)), [["zoxide", ["query", "-l"], { on: "wsl" }]]);
  state.query = "한글";
  state.input.change({}, cx);
  assert.equal(view.rows()[0].label, folder);
  state.input.submit();
  assert.equal(state.closed, true);
  assert.deepEqual(JSON.parse(JSON.stringify(state.commands.at(-1))), {
    command: "space.new",
    args: { name: "한글 project's $draft; notes", machine: "wsl", folder },
  });
});

test("an unavailable machine cannot fall back to a local directory or create a space", async () => {
  const { view, state } = await fixture("directories", true, new Error("machine unavailable"),
    { machine: "wsl" });
  await Promise.all(state.tasks);
  assert.match(view.error, /machine unavailable/);
  assert.equal(view.busy, false);
  assert.equal(state.executions.length, 1);
  state.input.submit();
  assert.equal(state.closed, false);
  assert.equal(state.commands.length, 0);
});
