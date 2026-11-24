import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import vm from "node:vm";

const source = await readFile(new URL("../files/mods/pi-editor/link.js", import.meta.url), "utf8");
test("a remote Pi link opens the editor on the clicked machine, not the desktop", async () => {
  const calls = [];
  const session = "01234567-89ab-cdef-0123-456789abcdef";
  const api = {
    terminals: () => [{ address: "sepia/t4", labels: { "pi.subagents": JSON.stringify({ session }) } }],
    exec: async (...args) => { calls.push(args); return { code: 0, stdout: "t9\n" }; },
    run: (...args) => calls.push(args),
  };
  const mod = new vm.SourceTextModule(source);
  await mod.link(() => new vm.SyntheticModule(Object.keys(api), function () {
    for (const [k, v] of Object.entries(api)) this.setExport(k, v);
  }));
  await mod.evaluate();
  const click = { link: { kind: "file", raw: "modules/pi/README.md#hindsight-memory" },
    source: { cwd: "/project", machine: "sepia", terminal: "sepia/t4", pane: "p1" } };
  assert.equal(await mod.namespace.handleLink(click), "handled");
  assert.deepEqual(calls, [["pi-session-editor", ["t4", session, click.link.raw], { cwd: "/project", on: "sepia" }],
    ["terminal.peek", { terminal: "sepia/t9" }]]);
  click.source.terminal = "sepia/t5";
  assert.equal(await mod.namespace.handleLink(click), "pass");
  assert.equal(calls.length, 2);
});
