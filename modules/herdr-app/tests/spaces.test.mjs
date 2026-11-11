import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { setImmediate } from "node:timers/promises";
import vm from "node:vm";

const source = await readFile(new URL("../files/mods/tui-spaces/main.js", import.meta.url), "utf8");

async function fixture() {
  const state = { agents: [], nodes: new Map(), commands: [] };
  function element() {
    const node = { content: [] };
    const proxy = new Proxy(node, { get(target, key) {
      if (key in target) return target[key];
      return (...args) => {
        if (key === "id") state.nodes.set(args[0], node);
        else if (key === "child") node.content.push(args[0]);
        else if (key === "children") node.content.push(...args[0]);
        else if (key === "when" && args[0]) args[1](proxy);
        else if (key === "bg") node.background = args[0];
        return proxy;
      };
    } });
    return proxy;
  }
  const rows = ["space", "remote/space"].map((id, i) => ({
    w: { id, name: "Work", status: "blocked", machine: i ? "remote" : null }, i,
  }));
  state.rows = rows;
  const api = {
    "gpui-kit": { View: class {}, div: element, svg: element },
    "gpui-base": { v_flex: element, h_flex: element },
    herdr: {
      agents: () => state.agents, spaces: () => state.rows.map(r => r.w), machines: () => [], panes: () => [],
      shared: () => null, setShared() {}, run: (command, args) => state.commands.push({ command, args }),
      openOverlay() {}, exec() {}, hc: async () => ({ code: 0, stdout: '{"machines":[]}' }),
      nextCommand: () => new Promise(resolve => { state.deliver = resolve; }),
      theme() {}, watchFolder() {}, window() {}, layout() {},
    },
  };
  const context = vm.createContext({ localStorage: { getItem: () => null, setItem() {} } });
  const module = new vm.SourceTextModule(source, { context });
  await module.link(name => new vm.SyntheticModule(Object.keys(api[name]), function () {
    for (const [key, value] of Object.entries(api[name])) this.setExport(key, value);
  }, { context }));
  await module.evaluate();
  const view = new module.namespace.default();
  const cx = { notify() {}, timer: { every() {} }, spawn: fn => { void fn(cx); } };
  view.init({}, cx);
  await setImmediate();
  view.ordered = () => state.rows;
  return { state, view, async command(name) {
    state.deliver(name);
    await setImmediate();
  }, draw(collapsed) {
    state.nodes.clear();
    view.grouped({ ansi4: "#2864c8" }, collapsed);
    return [...state.nodes.keys()].filter(id => id.startsWith("space-done-"));
  } };
}

test("completion survives blocked peers and clears only after the last unread agent, in both sidebar widths", async () => {
  const { state, draw } = await fixture();
  for (const collapsed of [false, true]) {
    state.agents = [
      { space: "space", status: "blocked" },
      { space: "space", status: "done" },
      { space: "space", status: "done" },
      { space: "remote/space", status: "working" },
    ];
    assert.deepEqual(draw(collapsed), ["space-done-space"]);
    assert.equal(state.nodes.get("space-done-space").background, "#2864c8");
    state.agents[1].status = "idle";
    assert.deepEqual(draw(collapsed), ["space-done-space"]);
    state.agents[2].status = "idle";
    assert.deepEqual(draw(collapsed), []);
    state.agents[3].status = "done";
    assert.deepEqual(draw(collapsed), ["space-done-remote/space"]);
    state.agents = [];
    assert.deepEqual(draw(collapsed), []);
  }
});

test("numbered selection and compact labels follow sidebar groups rather than core machine order", async () => {
  const { state, view, command, draw } = await fixture();
  state.rows = [
    { w: { id: "192.168.0.10/a", name: "Remote", machine: "192.168.0.10" }, i: 0 },
    { w: { id: "wsl/a", name: "WSL", machine: "wsl" }, i: 1 },
    { w: { id: "wsl/b", name: "Worktree", machine: "wsl" }, i: 2, tree: "└─" },
  ];
  view.machines = [{ name: "empty-machine", state: "unreachable" }];
  const expected = ["space-wsl/a", "space-wsl/b", "space-192.168.0.10/a"];
  for (const collapsed of [false, true]) {
    draw(collapsed);
    assert.deepEqual([...state.nodes.keys()].filter(id => id.startsWith("space-")), expected);
    if (collapsed) {
      const text = node => typeof node === "string" ? node : node.content.map(text).join("");
      expected.forEach((id, i) => assert.ok(text(state.nodes.get(id)).startsWith(String(i + 1))));
    }
  }
  for (const [number, id] of [[1, "wsl/a"], [2, "wsl/b"], [3, "192.168.0.10/a"]]) {
    await command(`select-${number}`);
    assert.deepEqual(JSON.parse(JSON.stringify(state.commands.at(-1))), { command: "space.select", args: { id } });
  }
  view.folded.add("wsl");
  draw(false);
  await command("select-3");
  assert.equal(state.commands.at(-1).args.id, "192.168.0.10/a", "folding does not renumber other machines");
  const count = state.commands.length;
  for (const invalid of ["select-9", "select-0", "select-10", "unrelated"]) await command(invalid);
  assert.equal(state.commands.length, count, "empty slots and unknown commands do not change focus");
});

test("numbered selection uses current spaces and reserves no slot for an empty local group", async () => {
  const { state, command } = await fixture();
  state.rows = [
    { w: { id: "192.168.0.10/a", name: "Remote", machine: "192.168.0.10" }, i: 0 },
    { w: { id: "wsl/a", name: "First", machine: "wsl" }, i: 1 },
    { w: { id: "wsl/b", name: "Second", machine: "wsl" }, i: 2 },
  ];
  await command("select-1");
  assert.equal(state.commands.at(-1).args.id, "wsl/a");
  state.rows.splice(1, 1);
  await command("select-1");
  assert.equal(state.commands.at(-1).args.id, "wsl/b");
  state.rows.unshift({ w: { id: "local", name: "Local", machine: null }, i: 0 });
  await command("select-1");
  assert.equal(state.commands.at(-1).args.id, "local");
  await command("select-2");
  assert.equal(state.commands.at(-1).args.id, "wsl/b");
});
