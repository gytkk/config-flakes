import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import vm from "node:vm";

const source = await readFile(new URL("../files/mods/pi-editor/link.js", import.meta.url), "utf8");
const manifest = JSON.parse(await readFile(new URL("../files/mods/pi-editor/mod.json", import.meta.url)));
test("resolve on the clicked Pi host, then split that pane with literal Neovim arguments", async () => {
  const calls = [];
  const path = "/project/it's a file; $name.nix";
  const api = {
    terminals: () => [{ address: "sepia/t4", report: { app: "pi" } }],
    exec: async (...args) => {
      calls.push(args);
      return args[1][0] === "missing" ? { code: 2 } : { code: 0, stdout: JSON.stringify({ path, line: 7 }) };
    },
    run: (...args) => calls.push(args),
  };
  const mod = new vm.SourceTextModule(source);
  await mod.link(() => new vm.SyntheticModule(Object.keys(api), function () {
    for (const [k, v] of Object.entries(api)) this.setExport(k, v);
  }));
  await mod.evaluate();
  const click = { link: { kind: "file", raw: "src/flake.nix#L7" },
    source: { cwd: "/project", machine: "sepia", terminal: "sepia/t4", pane: "sepia/p1" } };
  for (const name of ["src/flake.nix", "./Makefile", "./.gitignore"]) {
    assert.ok(manifest.link_handlers.some(r => r.kind === "file" && new RegExp(r.pattern).test(name)));
  }
  assert.equal(await mod.namespace.handleLink(click), "handled");
  assert.deepEqual(calls, [["pi-session-editor", [click.link.raw], { cwd: "/project", on: "sepia" }],
    ["pane.split", { pane: "sepia/p1", direction: "right", argv: ["nvim", "+7", "--", path] }]]);
  click.link.raw = "missing";
  assert.equal(await mod.namespace.handleLink(click), "pass");
  click.link.kind = "url";
  assert.equal(await mod.namespace.handleLink(click), "pass");
  click.link.kind = "file";
  click.source.terminal = "sepia/t5";
  assert.equal(await mod.namespace.handleLink(click), "pass");
  assert.equal(calls.length, 3);
});
