import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import vm from "node:vm";

const source = await readFile(new URL("../files/mods/tui-spaces/main.js", import.meta.url), "utf8");

async function fixture() {
  const state = { agents: [], nodes: new Map() };
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
  const api = {
    "gpui-kit": { View: class {}, div: element, svg: element },
    "gpui-base": { v_flex: element, h_flex: element },
    herdr: {
      agents: () => state.agents, spaces: () => rows.map(r => r.w), machines: () => [], panes: () => [],
      shared: () => null, setShared() {}, run() {}, openOverlay() {}, exec() {}, hc() {},
      theme() {}, watchFolder() {}, window() {}, layout() {},
    },
  };
  const context = vm.createContext({});
  const module = new vm.SourceTextModule(source, { context });
  await module.link(name => new vm.SyntheticModule(Object.keys(api[name]), function () {
    for (const [key, value] of Object.entries(api[name])) this.setExport(key, value);
  }, { context }));
  await module.evaluate();
  const view = new module.namespace.default();
  Object.assign(view, { ordered: () => rows, pressed: null, machines: [], reconnecting: {}, folded: new Set(), shown: new Set(rows.map(r => r.w.id)) });
  return { state, draw(collapsed) {
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
