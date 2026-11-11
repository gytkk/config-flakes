import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import vm from "node:vm";

const mainSource = await readFile(new URL("../files/mods/omp-agents/main.js", import.meta.url), "utf8");
const snapshotSource = await readFile(new URL("../files/mods/omp-agents/snapshot.js", import.meta.url), "utf8");
const now = 100_000;
const child = (id = "Scout", phase = "running") => ({ id, phase, model: "resolved/model", activity: "Reading source", ...(phase === "running" ? {} : { endedAt: now }) });
const snapshot = (changes = {}) => JSON.stringify({ version: 1, session: "session-one", updatedAt: now, model: "parent/model", children: [child()], ...changes });
const claudeChild = (id = "native-id", phase = "running") => ({
  id, name: "Source scout", phase, model: "resolved/claude-model", activity: "Inspecting Claude source", unavailable: false,
  ...(phase === "running" ? {} : phase === "stopping" ? { stoppedAt: now } : { endedAt: now }),
});
const claudeSnapshot = (changes = {}) => snapshot({ children: [claudeChild()], ...changes });
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
        else if (key === "bg") target.background = args[0];
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
  for (const raw of invalid) assert.equal(snapshots.snapshotChildren(raw, "t1", "omp", now).length, 0, String(raw));
  assert.equal(snapshots.snapshotChildren(snapshot(), "t1", "omp", now + 14_999).length, 1);
});

test("completed, failed and aborted children expire at five seconds without a seven-slot limit", async () => {
  const { snapshots } = await fixture();
  const children = [...Array.from({ length: 9 }, (_, index) => child(`Scout ${index}`)), ...["completed", "failed", "aborted"].map(phase => child(phase, phase))];
  const raw = snapshot({ children });
  assert.equal(snapshots.snapshotChildren(raw, "t1", "omp", now + 4_999).length, 12);
  assert.equal(snapshots.snapshotChildren(raw, "t1", "omp", now + 5_000).length, 9);
});

test("association and UI keys retain exact remote addresses and session identity", async () => {
  const { snapshots } = await fixture();
  const all = [terminal("t1"), terminal("machine/t1", snapshot({ session: "remote-session", children: [child("Remote")] }))];
  const children = snapshots.metadataChildren(all, now);
  assert.equal(children.get("t1")[0].name, "Scout");
  assert.equal(children.get("machine/t1")[0].name, "Remote");
  assert.equal(children.get("machine/t1")[0].parent, "machine/t1");
  assert.notEqual(children.get("t1")[0].address, children.get("machine/t1")[0].address);
  assert.notEqual(children.get("t1")[0].address, snapshots.snapshotChildren(snapshot({ session: "new-session" }), "t1", "omp", now)[0].address);
  all[1].state = "exited";
  assert.equal(snapshots.metadataChildren(all, now).has("machine/t1"), false);
});

test("unknown child models stay unknown rather than borrowing the parent model", async () => {
  const { snapshots } = await fixture();
  const visible = snapshots.snapshotChildren(snapshot({ children: [{ ...child(), model: "", activity: `\u001b[31m${"x".repeat(90)}\n` }] }), "t1", "omp", now)[0];
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
  assert.equal(f.state.nodes.get("sec-1").content.some(node => content(node) === "2"), true);
  f.state.nodes.get("fold-t1").click(null, f.cx);
  f.render();
  assert.equal(f.state.nodes.get("fold-t1").content.some(node => content(node) === "1"), true);
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

test("Claude snapshots reject malformed children atomically and retain the shared freshness policy", async () => {
  const { snapshots } = await fixture();
  const invalid = [
    claudeSnapshot({ version: 2 }), claudeSnapshot({ session: "" }), claudeSnapshot({ updatedAt: now + 1 }),
    claudeSnapshot({ updatedAt: now - 15_000 }), claudeSnapshot({ model: null }), claudeSnapshot({ children: {} }),
    claudeSnapshot({ children: [claudeChild(), claudeChild()] }),
    ...[
      { name: undefined }, { name: null }, { unavailable: undefined }, { unavailable: "false" },
      { phase: "aborted" }, { model: null }, { activity: null }, { id: " " },
      { endedAt: now }, { stoppedAt: now },
    ].map(changes => claudeSnapshot({ children: [claudeChild("valid-first"), { ...claudeChild(), ...changes }] })),
    ...[
      { stoppedAt: undefined }, { stoppedAt: now + 1 }, { stoppedAt: "100000" }, { endedAt: now },
    ].map(changes => claudeSnapshot({ children: [{ ...claudeChild("stop", "stopping"), ...changes }] })),
    ...[
      { endedAt: undefined }, { endedAt: now + 1 }, { endedAt: -1 }, { stoppedAt: now + 1 },
    ].map(changes => claudeSnapshot({ children: [{ ...claudeChild("done", "completed"), ...changes }] })),
  ];
  for (const raw of invalid) assert.equal(snapshots.snapshotChildren(raw, "machine/t1", "claude", now).length, 0, raw);
  assert.equal(snapshots.snapshotChildren(claudeSnapshot(), "machine/t1", "claude", now + 14_999).length, 1);
  assert.equal(snapshots.snapshotChildren(claudeSnapshot(), "machine/t1", "claude", now + 15_000).length, 0);
  assert.equal(snapshots.snapshotChildren(claudeSnapshot(), "machine/t1", "unknown", now).length, 0);
  assert.equal(snapshots.snapshotChildren(snapshot(), "machine/t1", "claude", now).length, 0);
  assert.equal(snapshots.snapshotChildren(claudeSnapshot({ children: [claudeChild("stop", "stopping")] }), "machine/t1", "omp", now).length, 0);
});

test("late stop-hook delivery does not reject or prolong a terminal Claude result", async () => {
  const { snapshots } = await fixture();
  const raw = claudeSnapshot({ children: [{ ...claudeChild("done", "completed"), endedAt: now - 9_999, stoppedAt: now }] });
  assert.equal(snapshots.snapshotChildren(raw, "t1", "claude", now).length, 1);
  assert.equal(snapshots.snapshotChildren(raw, "t1", "claude", now + 1).length, 0);
});

test("Claude uses separate sanitized display names and retains all stopping and terminal children for ten seconds", async () => {
  const { snapshots } = await fixture();
  const phases = ["stopping", "completed", "failed", "interrupted"];
  const children = [
    ...Array.from({ length: 9 }, (_, index) => claudeChild(`child-${index}`)),
    ...phases.map(phase => claudeChild(phase, phase)),
  ];
  const raw = claudeSnapshot({ children });
  assert.equal(snapshots.snapshotChildren(raw, "t1", "claude", now + 9_999).length, 13);
  assert.equal(snapshots.snapshotChildren(raw, "t1", "claude", now + 10_000).length, 9);
  const name = `\u001b[31m${"界".repeat(90)}\n`;
  const visible = snapshots.snapshotChildren(claudeSnapshot({ children: [{ ...claudeChild(), name, model: "", activity: name }] }), "t1", "claude", now)[0];
  assert.equal(visible.id, "native-id");
  assert.notEqual(visible.name, visible.id);
  assert.equal(visible.model, "");
  for (const value of [visible.name, visible.activity]) {
    assert.equal(Array.from(value).length, 80);
    assert.equal(value.includes("\u001b"), false);
    assert.equal(value.includes("\n"), false);
    assert.equal(value.endsWith("…"), true);
  }
});

test("runtime, exact parent, session and opaque child id all isolate metadata addresses", async () => {
  const { snapshots } = await fixture();
  const raw = claudeSnapshot({ children: [claudeChild("same-id"), { ...claudeChild("other-id"), name: "Source scout" }] });
  const all = ["t1", "machine/t1"].map(address => ({
    ...terminal(address, snapshot({ children: [child("same-id")] })),
    labels: { "omp.subagents": snapshot({ children: [child("same-id")] }), "claude.subagents": raw },
  }));
  const grouped = snapshots.metadataChildren(all, now);
  const visible = Array.from(grouped.values()).flat();
  assert.equal(visible.length, 6);
  assert.equal(new Set(visible.map(k => k.address)).size, 6);
  assert.deepEqual(Array.from(grouped.get("machine/t1"), k => k.parent), ["machine/t1", "machine/t1", "machine/t1"]);
  const original = grouped.get("t1").find(k => k.runtime === "claude");
  const restarted = snapshots.snapshotChildren(claudeSnapshot({ session: "owner.new-generation", children: [claudeChild("same-id")] }), "t1", "claude", now)[0];
  assert.notEqual(original.address, restarted.address);
  const ambiguous = snapshots.snapshotChildren(claudeSnapshot({ session: 'session-one","same-id', children: [claudeChild("other-id")] }), "t1", "claude", now)[0];
  assert.notEqual(original.address, ambiguous.address);
  all[1].state = "exited";
  assert.equal(snapshots.metadataChildren(all, now).has("machine/t1"), false);
  all[0].labels["claude.subagents"] = "{";
  assert.equal(snapshots.metadataChildren(all, now).get("t1").length, 1);
});

test("Claude lifecycle dots use the incumbent palette and uncertain states remain neutral", async () => {
  const f = await fixture();
  const phases = ["running", "stopping", "completed", "failed", "interrupted"];
  const children = [
    ...phases.map(phase => claudeChild(phase, phase)),
    ...["running", "completed", "failed"].map(phase => ({ ...claudeChild(`unavailable-${phase}`, phase), unavailable: true })),
  ];
  f.state.agents = [agent("t1")];
  f.state.terminals = [{ ...terminal("t1", ""), labels: { "claude.subagents": claudeSnapshot({ children }) } }];
  f.render();
  const rows = [...f.state.nodes].filter(([key]) => key.startsWith("sub-claude:")).map(([, row]) => row);
  assert.deepEqual(rows.map(row => row.content.at(-1).background), ["#00aa00", "#777777", "#0000ff", "#ff0000", "#777777", "#777777", "#777777", "#777777"]);
  const projected = f.snapshots.metadataChildren(f.state.terminals, now).get("t1");
  assert.deepEqual(Array.from(projected, k => [k.phase, k.status, k.unavailable]), [
    ["running", "working", false], ["stopping", "idle", false], ["completed", "done", false], ["failed", "blocked", false], ["interrupted", "idle", false],
    ["running", "idle", true], ["completed", "idle", true], ["failed", "idle", true],
  ]);
  assert.equal(new Set(rows.slice(0, 5).map(row => row.tip)).size, 5);
  for (let index = 0; index < rows.length; index++) {
    assert.equal(rows[index].tip.includes(children[index].model), true);
    assert.equal(rows[index].tip.includes(children[index].activity), true);
    assert.equal(content(rows[index]).includes(children[index].name), true);
    assert.equal(content(rows[index]).includes(children[index].id), false);
  }
  for (let index = 0; index < 3; index++) {
    const phaseIndex = phases.indexOf(children[index + 5].phase);
    assert.notEqual(rows[index + 5].tip, rows[phaseIndex].tip);
  }
});

test("both metadata runtimes coexist with native rows and preserve folding, focus and parent priority", async () => {
  const f = await fixture();
  f.state.agents = [agent("machine/t1"), agent("machine/t3", "working", 2), agent("machine/t4", "idle", 3)];
  f.state.terminals = [
    { ...terminal("machine/t1", snapshot({ children: [child("same-id", "failed")] })), labels: {
      "omp.subagents": snapshot({ children: [child("same-id", "failed")] }),
      "claude.subagents": claudeSnapshot({ children: [claudeChild("same-id", "failed")] }),
    } },
    { ...terminal("machine/t2", "", "machine/t1"), name: "Native peek", status: "idle" },
    { ...terminal("machine/t4", "", "machine/t1"), name: "Native pane", status: "idle" },
    terminal("machine/t3", ""),
  ];
  f.render();
  assert.deepEqual(f.state.order, ["machine/t3", "machine/t1"]);
  assert.equal(f.state.agents[0].status, "idle");
  assert.equal(f.state.nodes.get("sec-1").content.some(node => content(node) === "5"), true);
  const metadataRows = [...f.state.nodes].filter(([key]) => key.startsWith("sub-omp:") || key.startsWith("sub-claude:"));
  assert.equal(metadataRows.length, 2);
  for (const [, row] of metadataRows) row.click();
  f.state.nodes.get("sub-machine/t2").click();
  f.state.nodes.get("sub-machine/t4").click();
  assert.deepEqual(f.state.commands.map(({ command, args }) => [command, args.terminal]), [
    ["pane.focus", "machine/t1"], ["pane.focus", "machine/t1"], ["terminal.peek", "machine/t2"], ["pane.focus", "machine/t4"],
  ]);
  f.state.nodes.get("fold-machine/t1").click(null, f.cx);
  f.render();
  assert.equal([...f.state.nodes.keys()].some(key => key.startsWith("sub-")), false);
  assert.equal(f.state.nodes.get("fold-machine/t1").content.some(node => content(node) === "4"), true);
  f.state.nodes.get("fold-machine/t1").click(null, f.cx);
  f.render();
  assert.equal([...f.state.nodes.keys()].filter(key => key.startsWith("sub-")).length, 4);
  f.state.width = 76;
  f.render();
  assert.equal([...f.state.nodes.keys()].some(key => key.startsWith("sub-")), false);
  assert.deepEqual(f.state.order, ["machine/t3", "machine/t4", "machine/t1"]);
  f.state.nodes.get("ag-machine/t1").click();
  assert.equal(f.state.commands.at(-1).args.pane, "pane-machine/t1");
});

test("idle refresh expires Claude stopping and settled rows before the snapshot itself", async () => {
  const f = await fixture();
  f.state.agents = [agent("t1")];
  f.state.terminals = [{ ...terminal("t1"), labels: {
    "omp.subagents": snapshot({ children: [child("omp-done", "completed")] }),
    "claude.subagents": claudeSnapshot({ children: [
      claudeChild("running"), claudeChild("stopping", "stopping"), claudeChild("done", "completed"),
      claudeChild("failed", "failed"), claudeChild("interrupted", "interrupted"),
    ] }),
  } }];
  const count = prefix => [...f.state.nodes.keys()].filter(key => key.startsWith(prefix)).length;
  f.render();
  assert.equal(count("sub-omp:"), 1);
  assert.equal(count("sub-claude:"), 5);
  for (const [elapsed, ompCount, claudeCount] of [[5_000, 0, 5], [9_999, 0, 5], [10_000, 0, 1], [15_000, 0, 0]]) {
    f.state.now = now + elapsed;
    f.state.timers[0].handler(f.cx);
    f.render();
    assert.equal(count("sub-omp:"), ompCount);
    assert.equal(count("sub-claude:"), claudeCount);
  }
  assert.equal(f.state.notified, 4);
});
