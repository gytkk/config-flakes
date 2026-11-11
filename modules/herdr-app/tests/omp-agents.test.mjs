import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import vm from "node:vm";

const mainSource = await readFile(new URL("../files/mods/omp-agents/main.js", import.meta.url), "utf8");
const snapshotSource = await readFile(new URL("../files/mods/omp-agents/snapshot.js", import.meta.url), "utf8");
const now = 100_000;
const child = (id = "Scout", phase = "running") => ({ id, phase, model: "resolved/model", activity: "Reading source", ...(phase === "running" ? {} : { endedAt: now }) });
const snapshot = (changes = {}) => JSON.stringify({ version: 1, session: "session-one", updatedAt: now, model: "parent/model", children: [child()], ...changes });
const terminal = (address, raw = snapshot(), parent = null) => ({ address, parent, state: "live", labels: { "omp.subagents": raw } });

async function fixture() {
  const state = { now, commands: [], timers: [], nodes: new Map(), agents: [], terminals: [], order: [], width: 268, props: null, notified: 0 };
  const storage = new Map();
  const context = vm.createContext({
    Date: class extends Date { static now() { return state.now; } },
    localStorage: { getItem: key => storage.get(key) ?? null, setItem: (key, value) => storage.set(key, value), removeItem: key => storage.delete(key) },
  });
  function element() {
    const node = { children: [] };
    const proxy = new Proxy(node, { get(target, key) {
      if (key in target) return target[key];
      return (...args) => {
        if (key === "id") { target.id = args[0]; state.nodes.set(args[0], proxy); }
        else if (key === "child") target.children.push(args[0]);
        else if (key === "children") target.children.push(...args[0]);
        else if (key === "when" && args[0]) args[1](proxy);
        else if (key === "on_click") target.click = args[0];
        else if (key === "tooltip") target.tip = args[0];
        return proxy;
      };
    } });
    // children is both a builder method and retained content in the real kit.
    node.content = node.children;
    delete node.children;
    node.child = value => { node.content.push(value); return proxy; };
    node.children = values => { node.content.push(...values); return proxy; };
    return proxy;
  }
  const exports = {
    "gpui-kit": { View: class {}, div: element, image: element, svg: element },
    "gpui-base": { v_flex: element, h_flex: element },
    herdr: {
      agents: () => state.agents, terminals: () => state.terminals, panes: () => [],
      focused: () => null, spaces: () => [], theme: () => ({ dim: "#777777", muted: "#aaaaaa", working: "#00aa00", blocked: "#ff0000", done: "#0000ff" }),
      shared: key => key === "agents.sort" ? "priority" : null,
      setShared() {}, setAgentOrder: order => { state.order = Array.from(order); },
      run: (command, args) => state.commands.push({ command, args }), openOverlay: spec => { state.overlay = spec; }, closeOverlay() {},
      props: () => state.props, window: () => ({ platform: "macos" }), layout: () => ({ id: "sidebar", size: { px: state.width } }),
    },
  };
  const snapshots = new vm.SourceTextModule(snapshotSource, { context });
  await snapshots.link(() => { throw new Error("Snapshot helpers must remain host-independent"); });
  const module = new vm.SourceTextModule(mainSource, { context });
  await module.link(name => {
    if (name === "./snapshot.js") return snapshots;
    return new vm.SyntheticModule(Object.keys(exports[name]), function () {
      for (const [key, value] of Object.entries(exports[name])) this.setExport(key, value);
    }, { context });
  });
  await module.evaluate();
  const cx = { notify() { state.notified++; }, stop_propagation() {}, timer: { every(ms, handler) { state.timers.push({ ms, handler }); } } };
  const view = new module.namespace.default();
  view.init({}, cx);
  return { state, view, cx, snapshots: snapshots.namespace, render() { state.nodes.clear(); return view.render(cx); } };
}

function agent(address, status = "idle", seq = 1) {
  return { address, status, seq, pane: `pane-${address}`, space: "work", space_name: "Work", tab: `tab-${address}`, program: {}, title: "OMP", pinned: false };
}

function content(node) {
  if (typeof node === "string") return node;
  return (node?.content ?? []).map(content).join(" ");
}

test("invalid, stale, future and malformed full snapshots are ignored", async () => {
  const { snapshots } = await fixture();
  const invalid = [undefined, "{", "null", snapshot({ version: 2 }), snapshot({ session: "" }), snapshot({ session: 1 }),
    snapshot({ updatedAt: now - 15_000 }), snapshot({ updatedAt: now + 1 }), snapshot({ updatedAt: "100000" }),
    snapshot({ children: {} }), snapshot({ model: null }), snapshot({ children: [child(), child()] }),
    snapshot({ children: [{ ...child(), phase: "done" }] }), snapshot({ children: [{ ...child(), activity: null }] }),
    snapshot({ children: [{ ...child(), endedAt: now }] }), snapshot({ children: [{ ...child("Scout", "completed"), endedAt: undefined }] }),
    snapshot({ children: [{ ...child("Scout", "failed"), endedAt: now + 1 }] })];
  for (const raw of invalid) assert.equal(snapshots.snapshotChildren(raw, "t1", now).length, 0, String(raw));
  assert.equal(snapshots.snapshotChildren(snapshot(), "t1", now + 14_999).length, 1);
});

test("completed, failed and aborted children expire at five seconds without a seven-slot limit", async () => {
  const { snapshots } = await fixture();
  const children = [...Array.from({ length: 9 }, (_, index) => child(`Scout ${index}`)), ...["completed", "failed", "aborted"].map(phase => child(phase, phase))];
  const raw = snapshot({ children });
  assert.equal(snapshots.snapshotChildren(raw, "t1", now + 4_999).length, 12);
  assert.equal(snapshots.snapshotChildren(raw, "t1", now + 5_000).length, 9);
});

test("association and UI keys retain exact remote addresses and session identity", async () => {
  const { snapshots } = await fixture();
  const all = [terminal("t1"), terminal("machine/t1", snapshot({ session: "remote-session", children: [child("Remote")] }))];
  const children = snapshots.metadataChildren(all, now);
  assert.equal(children.get("t1")[0].name, "Scout");
  assert.equal(children.get("machine/t1")[0].name, "Remote");
  assert.equal(children.get("machine/t1")[0].parent, "machine/t1");
  assert.notEqual(children.get("t1")[0].address, children.get("machine/t1")[0].address);
  assert.notEqual(children.get("t1")[0].address, snapshots.snapshotChildren(snapshot({ session: "new-session" }), "t1", now)[0].address);
  all[1].state = "exited";
  assert.equal(snapshots.metadataChildren(all, now).has("machine/t1"), false);
});

test("unknown child models stay unknown rather than borrowing the parent model", async () => {
  const { snapshots } = await fixture();
  const visible = snapshots.snapshotChildren(snapshot({ children: [{ ...child(), model: "", activity: `\u001b[31m${"x".repeat(90)}\n` }] }), "t1", now)[0];
  assert.equal(visible.model, "");
  assert.equal(Array.from(visible.activity).length, 80);
  assert.equal(visible.activity.includes("\u001b"), false);
  assert.equal(visible.activity.endsWith("…"), true);
});

test("virtual rows focus only their actual parent while native children still peek", async () => {
  const f = await fixture();
  f.state.agents = [agent("machine/t1")];
  f.state.terminals = [terminal("machine/t1"), { ...terminal("machine/t2", undefined, "machine/t1"), labels: {}, name: "Native child", status: "working" }];
  f.render();
  const virtual = [...f.state.nodes].find(([key]) => key.startsWith("sub-omp:"))[1];
  virtual.click();
  f.state.nodes.get("sub-machine/t2").click();
  assert.deepEqual(f.state.commands.map(({ command, args }) => [command, args.terminal]), [["pane.focus", "machine/t1"], ["terminal.peek", "machine/t2"]]);
  assert.match(virtual.tip, /resolved\/model/);
  assert.match(virtual.tip, /Reading source/);
});

test("metadata does not change parent status, priority or navigation and counts include children", async () => {
  const f = await fixture();
  f.state.agents = [agent("t1", "idle"), agent("t2", "working", 2)];
  f.state.terminals = [terminal("t1", snapshot({ children: [child("Failure", "failed")] })), terminal("t2", "")];
  f.render();
  assert.deepEqual(f.state.order, ["t2", "t1"]);
  assert.equal(f.state.agents[0].status, "idle");
  assert.match(content(f.state.nodes.get("sec-1")), /Idle 2/);
  f.view.folded.add("t1");
  f.render();
  assert.match(content(f.state.nodes.get("fold-t1")), /1/);
  assert.equal([...f.state.nodes.keys()].some(key => key.startsWith("sub-omp:")), false);
  f.state.nodes.get("ag-sort").click();
  assert.equal(f.state.overlay.mod, "omp-agents");
  assert.equal(f.state.overlay.props.sortMenu, true);
});

test("idle timer expires snapshots and compact rail keeps only genuine parent navigation", async () => {
  const f = await fixture();
  f.state.agents = [agent("t1")];
  f.state.terminals = [terminal("t1")];
  f.render();
  assert.equal([...f.state.nodes.keys()].some(key => key.startsWith("sub-omp:")), true);
  f.state.now += 15_000;
  f.state.timers[0].handler(f.cx);
  assert.equal(f.state.notified, 1);
  f.render();
  assert.equal([...f.state.nodes.keys()].some(key => key.startsWith("sub-omp:")), false);
  f.state.now = now;
  f.state.width = 76;
  f.render();
  assert.equal([...f.state.nodes.keys()].some(key => key.startsWith("sub-omp:")), false);
  assert.deepEqual(f.state.order, ["t1"]);
});
