// Exact release, real Node/jiti + SDK lifecycle, no model/provider or ambient state.
import assert from "node:assert/strict";
import childProcess from "node:child_process";
import { createRequire, syncBuiltinESMExports } from "node:module";
import { readFileSync, writeFileSync, mkdirSync, existsSync } from "node:fs";
import { join, basename, dirname } from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";
const root = process.argv[3], cwd = join(root, "project"), agentDir = process.env.PI_CODING_AGENT_DIR;
// Observe only this fixture's children. Never inspect or kill ambient servers.
const children = [];
const originalSpawn = childProcess.spawn;
let onRenameRequest;
childProcess.spawn = function(command, args, options) {
  const child = originalSpawn.call(this, command, args, options);
  children.push({ command: String(command), args, child });
  if (child.stdin) {
    const write = child.stdin.write;
    child.stdin.write = function(chunk, ...rest) {
      const result = write.call(this, chunk, ...rest);
      if (String(chunk).includes('"method":"textDocument/rename"')) onRenameRequest?.();
      return result;
    };
  }
  return child;
};
syncBuiltinESMExports();
const pi = await import(pathToFileURL(process.argv[2]));
pi.initTheme("light", false);
const { theme } = await import(pathToFileURL(join(dirname(process.argv[2]), "modes/interactive/theme/theme.js")));
const { visibleWidth, stripTerminalSequences } = await import(pathToFileURL(createRequire(process.argv[2]).resolve("@earendil-works/pi-tui")));
const widgets = new Map();
function put(file, content) { mkdirSync(join(cwd, file, ".."), { recursive: true }); writeFileSync(join(cwd, file), content); }
mkdirSync(join(cwd, ".git"));
put("package.json", '{"type":"module","scripts":{"test":"touch TEST-RAN"}}');
put("tsconfig.json", '{"compilerOptions":{"strict":true,"module":"NodeNext","moduleResolution":"NodeNext"},"include":["*.ts"]}');
put("a.ts", 'export const answer: number = "wrong";\n');
put("b.ts", 'import { answer } from "./a.js";\nconsole.log(answer);\n');
const errors = [];
const runtime = await pi.createAgentSessionRuntime(async ({ cwd, sessionManager, sessionStartEvent }) => {
  const settingsManager = pi.SettingsManager.create(cwd, agentDir);
  const modelRuntime = await pi.ModelRuntime.create({ authPath: join(agentDir, "auth.json"), modelsPath: join(agentDir, "models.json"), allowModelNetwork: false });
  const services = await pi.createAgentSessionServices({ cwd, agentDir, settingsManager, modelRuntime,
    resourceLoaderOptions: { noContextFiles: true } });
  const loader = services.resourceLoader;
  assert.deepEqual(loader.getExtensions().errors, []);
  assert.deepEqual(loader.getExtensions().warnings ?? [], []);
  assert.equal(loader.getExtensions().extensions.length, 2);
  assert.equal(loader.getExtensions().extensions.filter(e => e.path.endsWith("pi-lens/dist/index.js")).length, 1);
  assert.deepEqual(loader.getSkills().skills, []);
  assert.deepEqual(loader.getPrompts().prompts, []);
  return { ...await pi.createAgentSessionFromServices({ services, sessionManager, sessionStartEvent }), services, diagnostics: services.diagnostics };
}, { cwd, agentDir, sessionManager: pi.SessionManager.inMemory(cwd) });
let session = runtime.session;
async function cleanup() {
  try { await Promise.race([runtime.dispose(), new Promise(resolve => setTimeout(resolve, 3000))]); }
  finally {
    // Only ChildProcess handles captured by this disposable host, never PID scans.
    for (const { child } of children) {
      if (child.exitCode === null && child.signalCode === null) child.kill("SIGKILL");
    }
  }
}
const watchdog = setTimeout(() => {
  console.error("Lens fixture exceeded 180 seconds");
  void cleanup().finally(() => process.exit(124));
}, 180_000);
watchdog.unref();
process.once("SIGTERM", () => { void cleanup().finally(() => process.exit(124)); });
async function bind() {
  session = runtime.session;
  await session.bindExtensions({
    mode: "tui", onError: e => errors.push(e),
    uiContext: {
      ...session.extensionRunner.createToolContext("widget-host", undefined).ui,
      theme,
      setWidget(name, factory) {
        if (factory) widgets.set(name, factory({ requestRender() {} }, theme));
        else widgets.delete(name);
      },
    },
  });
  const names = session.getAllTools().map(t => t.name);
  for (const n of ["lsp_navigation", "lens_diagnostics", "ast_grep_search", "ast_grep_replace", "ast_grep_outline"]) assert.equal(names.filter(x => x === n).length, 1);
  for (const n of ["lsp_hover", "lsp_rename", "lsp_apply_edit", "lsp_code_actions", "ast_search", "ast_preview", "ast_apply", "ast_discard", "analyze", "project_scan", "rebuild"]) assert.ok(!names.includes(n), n);
  session.setActiveToolsByName(names);
}
const text = r => r.content.filter(b => b.type === "text").map(b => b.text).join("\n");
let toolSequence = 0;
async function tool(name, args, quiet = false) {
  const toolCallId = `fixture-${++toolSequence}-${name}`;
  const ctx = session.extensionRunner.createToolContext(toolCallId, undefined);
  // Model-free seam: real registered execute and Pi hook dispatch, not a provider turn.
  const call = { type: "tool_call", toolName: name, toolCallId, input: args };
  const gate = await session.extensionRunner.emitToolCall(call);
  assert.ok(!gate?.block, gate?.reason);
  const definition = session.extensionRunner.getToolDefinition(name) ?? (name === "write" ? pi.createWriteTool(cwd) : name === "edit" ? pi.createEditTool(cwd) : undefined);
  assert.ok(definition, name);
  const raw = await definition.execute(call.toolCallId, args, undefined, undefined, ctx);
  const patch = await session.extensionRunner.emitToolResult({ ...call, type: "tool_result", ...raw, isError: !!raw.isError });
  const result = { ...raw, isError: !!raw.isError, ...patch };
  if (!quiet || result.isError) console.log(name, JSON.stringify(args), JSON.stringify(result));
  return result;
}
async function nav(operation, path, line, character, extra = {}) {
  return tool("lsp_navigation", { operation, path, line, character, ...extra });
}
function configEnvelope(r) { return JSON.parse(text(r).split("\n").find(l => l.startsWith('{"cwd"'))); }
const wait = ms => new Promise(r => setTimeout(r, ms));
async function stopped(previous, phase) {
  for (let i = 0; i < 50 && previous.some(p => p.child.exitCode === null && p.child.signalCode === null); i++) await wait(100);
  assert.deepEqual(previous.filter(p => p.child.exitCode === null && p.child.signalCode === null).map(p => p.command), [], phase);
  console.log("process teardown", phase, previous.length);
}
const servers = () => children.filter(p => /language-server|gopls|nixd|ty$|rust-analyzer/.test(p.command) || p.args?.some(a => a.endsWith("lens-stalled-server.mjs")));
try {
  await bind();
  await wait(300);
  assert.equal(servers().length, 0, "minimal startup must not launch servers");
  assert.equal(readFileSync(join(cwd, "a.ts"), "utf8"), 'export const answer: number = "wrong";\n');
  assert.ok(!existsSync(join(cwd, "TEST-RAN")));
  const effective = configEnvelope(await tool("effective_config", { path: "a.ts" }, true));
  assert.deepEqual(effective.recordCounts, {});
  assert.deepEqual(effective.file.tools.filter(t => t.selected).map(t => t.id), ["lsp"]);
  console.log("single package; no bundled skills/prompts; minimal startup; valid conservative config");
  // Real TypeScript diagnostics and navigation, including the Nix wrapper's bundled tsserver.
  let r = await nav("hover", "a.ts", 1, 14);
  assert.equal(r.isError, false); assert.match(text(r), /answer/);
  r = await tool("lens_diagnostics", { path: "a.ts", source: "lsp" });
  assert.equal(r.details.unconfirmed, false); assert.ok(r.details.totalDiagnostics > 0);
  const widget = widgets.get("pi-lens");
  assert.ok(widget, "Lens mounts its own status widget");
  for (const width of [40, 80, 160]) {
    const lines = widget.render(width);
    assert.equal(lines.length, 1, "compact widget has one summary row");
    assert.ok(visibleWidth(lines[0]) <= width, "summary stays within terminal width");
  }
  console.log("Lens compact widget:", stripTerminalSequences(widget.render(80)[0]));
  r = await nav("definition", "b.ts", 2, 13); assert.equal(r.isError, false); assert.ok(r.details.resultCount > 0);
  r = await nav("references", "a.ts", 1, 14); assert.ok(r.details.resultCount >= 2);
  r = await nav("codeAction", "a.ts", 1, 14); assert.equal(r.isError, false);
  // Built-in edit through Pi's real hook dispatcher (writes are covered below).
  // No formatter, fixer or test runner may execute.
  const fixed = 'export const answer: number=42;\n';
  r = await tool("edit", { path: "a.ts", edits: [{ oldText: 'export const answer: number = "wrong";\n', newText: fixed }] });
  assert.ok(!r.isError); assert.match(text(r), /JavaScript\/TypeScript clean/);
  r = await tool("lens_diagnostics", { path: "a.ts", source: "lsp" });
  assert.equal(r.details.unconfirmed, false); assert.equal(r.details.totalDiagnostics, 0);
  assert.equal(readFileSync(join(cwd, "a.ts"), "utf8"), fixed);
  assert.ok(!existsSync(join(cwd, "TEST-RAN")));
  r = await nav("rename", "a.ts", 1, 14, { newName: "renamed", apply: false });
  assert.equal(r.isError, false); assert.match(text(r), /b.ts/);
  assert.equal(readFileSync(join(cwd, "a.ts"), "utf8"), fixed);
  // Lens apply asks the server again. A changed file between calls is NOT a stale-token refusal.
  put("a.ts", fixed + "// intervening edit retained\n");
  r = await nav("rename", "a.ts", 1, 14, { newName: "renamed", apply: true });
  assert.equal(r.isError, false);
  assert.match(readFileSync(join(cwd, "a.ts"), "utf8"), /renamed.*\n\/\/ intervening edit retained/);
  assert.match(readFileSync(join(cwd, "b.ts"), "utf8"), /renamed/);
  console.log("TypeScript: error/fix, hover/definition/references, actions, multi-file rename; preview not byte-fixed");
  const before = readFileSync(join(cwd, "b.ts"), "utf8");
  r = await tool("ast_grep_search", { pattern: "console.log($A)", lang: "typescript", paths: ["b.ts"] }); assert.equal(r.details.matchCount, 1);
  r = await tool("ast_grep_outline", { paths: ["a.ts"] }); assert.equal(r.isError, false);
  const replacement = { pattern: "console.log($A)", rewrite: "console.info($A)", lang: "typescript", paths: ["b.ts"] };
  r = await tool("ast_grep_replace", { ...replacement, apply: false }); assert.equal(r.details.matchCount, 1); assert.equal(r.details.applied, false);
  r = await tool("ast_grep_replace", { ...replacement, apply: true }); assert.equal(r.isError, true); assert.match(text(r), /no-autofix/);
  assert.equal(readFileSync(join(cwd, "b.ts"), "utf8"), before);
  console.log("AST search/outline/preview; apply refused without changing bytes");
  // Hold Pi's real mutation queue while the real TS server answers rename.
  // A change DURING this apply request must be refused, unlike a change between calls.
  const ready = Promise.withResolvers(), release = Promise.withResolvers(), requested = Promise.withResolvers();
  const lock = pi.withFileMutationQueue(join(cwd, "a.ts"), async () => { ready.resolve(); await release.promise; });
  await ready.promise;
  const sourceBeforeConflict = readFileSync(join(cwd, "a.ts"), "utf8");
  onRenameRequest = () => requested.resolve();
  const applyConflict = nav("rename", "a.ts", 1, 14, { newName: "conflicting", apply: true });
  try {
    await Promise.race([requested.promise, wait(5000).then(() => { throw new Error("rename request not observed"); })]);
    put("a.ts", sourceBeforeConflict + "// concurrent writer\n");
  } finally { onRenameRequest = undefined; release.resolve(); await lock; }
  r = await applyConflict;
  assert.equal(r.isError, true); assert.match(text(r), /changed|stale/i);
  assert.equal(readFileSync(join(cwd, "b.ts"), "utf8"), before);
  assert.match(readFileSync(join(cwd, "a.ts"), "utf8"), /concurrent writer/);
  // Native Lens result renderer remains inside the generic frame, expandable.
  const definition = session.extensionRunner.getToolDefinition("ast_grep_replace");
  const renderers = session.extensionRunner.resolveToolRenderers("ast_grep_replace", () => definition);
  const row = new pi.ToolExecutionComponent("ast_grep_replace", "render", replacement, { outputPad: 1 }, renderers, { requestRender() {} }, cwd);
  const preview = await tool("ast_grep_replace", { ...replacement, apply: false }, true);
  row.updateResult(preview);
  assert.ok(row.render(80).some(l => l.includes("╭")));
  row.setExpanded(true);
  assert.ok(row.render(100).some(l => l.includes("console.info")));
  console.log("rename concurrent-write refusal; package preview expansion inside rounded frame");
  // Every available primary server gets an isolated project, an intentional error,
  // correction, and symbol navigation. Never run a package manager/build download.
  const languages = [
    { name: "Nix", binary: "nixd", file: "nix/main.nix", marker: "nix/flake.nix", manifest: '{ outputs = { self }: {}; }', bad: 'let answer = 42; in answer + missing\n', good: 'let answer = builtins.length []; in answer\n', line: 1, character: 37, hoverCharacter: 24 },
    { name: "Go", binary: "gopls", versionArgs: ["version"], file: "go/main.go", marker: "go/go.mod", manifest: 'module fixture\n\ngo 1.22\n', bad: 'package fixture\nvar answer int = "wrong"\nfunc Result() int { return answer }\n', good: 'package fixture\nvar answer int = 42\nfunc Result() int { return answer }\n', line: 3, character: 28 },
    { name: "Python ty", binary: "ty", file: "python/main.py", marker: "python/pyproject.toml", manifest: '[project]\nname="fixture"\nversion="0.0.0"\n', bad: 'answer: int = "wrong"\nresult = answer\n', good: 'answer: int = 42\nresult = answer\n', line: 2, character: 10 },
    { name: "Rust", binary: "rust-analyzer", file: "rust/src/lib.rs", marker: "rust/Cargo.toml", manifest: '[package]\nname="fixture"\nversion="0.1.0"\nedition="2021"\n', bad: 'pub fn answer() -> i32 { "wrong" }\npub fn result() -> i32 { answer() }\n', good: 'pub fn answer() -> i32 { 42 }\npub fn result() -> i32 { answer() }\n', line: 2, character: 25 },
  ];
  for (const language of languages) {
    const available = childProcess.spawnSync(language.binary, language.versionArgs ?? ["--version"], { encoding: "utf8", timeout: 10000 });
    if (available.status !== 0) { console.log(`UNVERIFIED ${language.name}: ${available.error ?? available.stderr}`); continue; }
    put(language.marker, language.manifest); put(language.file, language.bad);
    r = await tool("lens_diagnostics", { path: language.file, source: "lsp" });
    assert.equal(r.details.unconfirmed, false, language.name); assert.ok(r.details.totalDiagnostics > 0, language.name);
    await tool("write", { path: language.file, content: language.good });
    for (const operation of ["hover", "definition", "references"]) {
      r = await nav(operation, language.file, language.line, operation === "hover" ? language.hoverCharacter ?? language.character : language.character);
      assert.equal(r.isError, false, language.name + " " + operation);
      if (language.name === "Nix" && operation === "hover" && !r.details.resultCount) {
        console.log("UNVERIFIED Nix hover: supported server returned no result");
      } else {
        assert.ok(r.details.resultCount > 0, language.name + " " + operation);
      }
    }
    r = await tool("lens_diagnostics", { path: language.file, source: "lsp" });
    assert.equal(r.details.unconfirmed, false, language.name); assert.equal(r.details.totalDiagnostics, 0, language.name);
    assert.equal(readFileSync(join(cwd, language.file), "utf8"), language.good);
    console.log(`VERIFIED ${language.name}: error/fix; navigation calls completed (empty results noted separately)`);
  }
  // Audit every former extension route without starting unrelated language servers.
  const routes = { nix: "nix", go: "go", ts: "typescript", tsx: "typescript", js: "typescript", jsx: "typescript", mts: "typescript", cts: "typescript", mjs: "typescript", cjs: "typescript", json: "json", jsonc: "json", css: "css", scss: "css", less: "css", html: "html", htm: "html", lua: "lua", sh: "bash", bash: "bash", zsh: "shuck", tf: "terraform", tfvars: "terraform", py: "python", pyi: "python", yaml: "yaml", yml: "yaml", md: "marksman", markdown: null, toml: "toml", rs: "rust" };
  for (const [suffix, expected] of Object.entries(routes)) {
    const path = `routes/file.${suffix}`; put(path, "");
    const e = configEnvelope(await tool("effective_config", { path }, true));
    const primaries = e.file.servers.filter(s => s.selected && s.role === "language").map(s => s.id);
    if (expected) assert.ok(primaries.includes(expected), `${suffix}: ${primaries}`);
    else assert.deepEqual(primaries, [], suffix);
    assert.ok(e.file.tools.filter(t => t.selected).every(t => t.id === "lsp"), suffix);
  }
  console.log("AUDITED routes: Nix/Go/JS/TS/JSON(C)/CSS/SCSS/Less/HTML/Lua/Bash/Terraform/Python/YAML/Markdown/TOML/Rust; .zsh -> shuck, .markdown -> no server");
  const unexpected = children.filter(p => !(p.command === "npm" && p.args?.join(" ") === "config get prefix")).filter(p => /^(npm|npx|pip|uv|prettier|ruff|gofmt|rustfmt|eslint|knip|madge|jscpd)$/.test(basename(p.command))).map(({ command, args }) => ({ command, args }));
  assert.deepEqual(unexpected, [], "unsolicited installer/formatter/analyzer");
  assert.ok(!existsSync(join(cwd, "TEST-RAN")));
  // Actual /reload flow, then actual SDK new-session replacement and termination.
  let previous = servers();
  await session.reload();
  await stopped(previous, "reload");
  session.setActiveToolsByName(session.getAllTools().map(t => t.name));
  r = await nav("hover", "a.ts", 1, 14); assert.equal(r.isError, false);
  previous = servers();
  await runtime.newSession(); await bind(); await stopped(previous, "new session");
  r = await nav("hover", "a.ts", 1, 14); assert.equal(r.isError, false);
  // Only this disposable project is inspected. Project mutation toggles really can
  // override machine defaults; CLI disabling flags still win. Never claim a sandbox.
  put(".pi-lens.json", JSON.stringify({ format: { enabled: true }, autofix: { enabled: true }, actionableWarnings: { autoFix: { enabled: true } }, tests: { enabled: true }, startup: { scans: { enabled: true } },
    lsp: { servers: { missing: { extensions: [".lensmissing"], command: ["lens-fixture-missing-binary"] }, stalled: { extensions: [".lensstalled"], command: [process.execPath, fileURLToPath(new URL("./lens-stalled-server.mjs", import.meta.url))] } } } }));
  await session.reload(); session.setActiveToolsByName(session.getAllTools().map(t => t.name));
  session.extensionRunner.setFlagValue("no-autoformat", true);
  const overridden = configEnvelope(await tool("effective_config", { path: "b.ts" }, true));
  for (const key of ["/format/enabled", "/autofix/enabled", "/actionableWarnings/autoFix/enabled"]) assert.equal(overridden.provenance.find(p => p.key === key)?.tier, "project");
  // effective_config reports file provenance, including ignored global-only inputs;
  // it is not proof that the flag accessor honored them. Observe execution below.
  console.log("override provenance", overridden.recordCounts);
  r = await tool("ast_grep_replace", { ...replacement, apply: false }); assert.equal(r.details.matchCount, 1);
  r = await tool("ast_grep_replace", { ...replacement, apply: true }); assert.equal(r.isError, false); assert.equal(r.details.applied, true);
  assert.match(readFileSync(join(cwd, "b.ts"), "utf8"), /console.info/);
  session.extensionRunner.setFlagValue("no-autofix", true);
  r = await tool("ast_grep_replace", { ...replacement, apply: true }); assert.equal(r.isError, true); assert.match(text(r), /no-autofix/);
  put("file.lensmissing", "fixture");
  r = await nav("hover", "file.lensmissing", 1, 1); assert.equal(r.isError, true); assert.equal(r.details.failureKind, "no_server");
  r = await tool("lens_diagnostics", { path: "file.lensmissing", source: "lsp" }); assert.equal(r.details.unconfirmed, true);
  assert.ok(!existsSync(join(cwd, "TEST-RAN")));
  console.log("project mutation overrides confirmed; no automatic test execution; CLI no-autofix wins; missing binary unavailable (no install)");
  put("file.lensstalled", "fixture");
  r = await nav("hover", "file.lensstalled", 1, 1); assert.equal(r.isError, true); assert.equal(r.details.failureKind, "timeout");
  r = await tool("lens_diagnostics", { path: "file.lensstalled", source: "lsp" });
  assert.ok(r.isError || r.details.unconfirmed || r.details.timedOut, "silent server must never be clean");
  console.log("unresponsive server: navigation timeout and unconfirmed diagnostics");
  assert.deepEqual(errors, []);
} finally {
  try {
    await runtime.dispose();
    await stopped(servers(), "quit");
  } finally {
    clearTimeout(watchdog);
    for (const { child } of children) {
      if (child.exitCode === null && child.signalCode === null) child.kill("SIGKILL");
    }
  }
}
console.log("LENS CHECKS PASSED");
