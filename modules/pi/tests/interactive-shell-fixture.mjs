// Executed only in a disposable process by interactive-shell.test.ts. No model/provider.
import assert from "node:assert/strict";
import { readFileSync, writeFileSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { pathToFileURL } from "node:url";
import { createRequire } from "node:module";
import { spawnSync } from "node:child_process";
import { setTimeout as delay } from "node:timers/promises";
const [piEntry, root] = process.argv.slice(2);
assert.equal(process.platform, "linux", "This fixture establishes Linux PTY evidence, not macOS evidence");
const pi = await import(pathToFileURL(piEntry));
const require = createRequire(piEntry);
const { visibleWidth, stripTerminalSequences } = await import(pathToFileURL(require.resolve("@earendil-works/pi-tui")));
// Any accidental semantic/provider transmission fails rather than reaching the network.
globalThis.fetch = async () => { throw new Error("External calls are forbidden in the shell fixture"); };
pi.initTheme("light", false);
const { theme } = await import(pathToFileURL(join(dirname(piEntry), "modes/interactive/theme/theme.js")));
const cwd = join(root, "project"), agent = join(root, "agent");
const notices = [], entries = [], widgets = new Map(), pids = new Set(), pidStarts = new Map();
let loaded, tools, shell, active, overlay;
const tui = { terminal: { columns: 100, rows: 32 }, requestRender() {} };
const ui = {
  theme, notify(message, level) { assert.notEqual(level, "error", message); },
  onTerminalInput() { return () => {}; },
  setWidget(name, factory) { if (factory) widgets.set(name, factory(tui, theme)); else widgets.delete(name); },
  custom(factory, options) {
    return new Promise(resolve => {
      const done = value => { const old = overlay; overlay = undefined; old?.dispose?.(); resolve(value); };
      overlay = factory(tui, theme, { matches() { return false; } }, done);
      overlay.focused = true;
      options?.onHandle?.({ focus() {}, unfocus() {}, isFocused: () => true });
    });
  },
};
const ctx = { cwd, hasUI: true, mode: "tui", ui, isProjectTrusted: () => false,
  sessionManager: { getSessionId: () => "fixture", getSessionFile: () => undefined, getBranch: () => entries } };
async function emit(type, reason, extra = {}) {
  for (const extension of loaded.extensions) for (const handler of extension.handlers.get(type) ?? []) {
    await handler({ type, reason, ...extra }, ctx);
  }
}
async function load(reason = "startup") {
  loaded = await pi.discoverAndLoadExtensions([], cwd, agent);
  assert.deepEqual(loaded.errors, []); assert.deepEqual(loaded.warnings ?? [], []);
  assert.equal(loaded.extensions.length, 3);
  const registrations = loaded.extensions.flatMap(e => [...e.tools]);
  assert.equal(registrations.filter(([name]) => name === "interactive_shell").length, 1);
  assert(!registrations.some(([name]) => /^(bash|bash_tasks|bash_output|bash_kill|bash_watch)$/.test(name)));
  assert(!loaded.extensions.some(e => e.commands.has("bg") || e.shortcuts.has("ctrl+shift+b")));
  tools = new Map(registrations.map(([name, tool]) => [name, tool.definition]));
  active = ["bash", ...tools.keys()];
  Object.assign(loaded.runtime, {
    getActiveTools: () => active, setActiveTools: names => { active = names; },
    getAllTools: () => [{ name: "bash", sourceInfo: { source: "builtin" } }, ...registrations.map(([name, tool]) => ({ name, sourceInfo: tool.sourceInfo }))],
    appendEntry: (customType, data) => entries.push({ type: "custom", customType, data }),
    sendMessage: (message, options) => { notices.push({ message, options }); },
  });
  await emit("session_start", reason);
  assert(active.includes("enable_interactive_shell")); assert(!active.includes("interactive_shell"));
  shell = tools.get("interactive_shell");
  assert(!("outputSelection" in shell.parameters.properties), "lean schema");
  assert.equal(shell.promptSnippet, undefined, "deferred prompt remains lean");
  await tools.get("enable_interactive_shell").execute("enable", {}, undefined, undefined, ctx);
  assert(active.includes("interactive_shell"));
}
let callId = 0;
async function call(args) {
  return shell.execute(`fixture-${++callId}`, shell.prepareArguments(args), undefined, undefined, ctx);
}
const text = result => result.content.map(c => c.text ?? "").join("\n");
async function until(predicate, label, ms = 6000) {
  const end = Date.now() + ms;
  while (!predicate()) { assert(Date.now() < end, `Timed out: ${label}`); await delay(25); }
}
function completion(id) { return notices.find(n => n.message.customType === "interactive-shell-transfer" && n.message.details?.sessionId === id); }
function processState(pid) {
  const fields = readFileSync(`/proc/${pid}/stat`, "utf8").split(") ")[1].split(" ");
  return { state: fields[0], start: fields[19] };
}
function alive(pid) {
  try { const current = processState(pid); return current.state !== "Z" && current.start === pidStarts.get(pid); }
  catch { return false; }
}
function liveOwnedGroups() {
  const result = spawnSync("ps", ["-eo", "pid=,pgid=,stat="], { encoding: "utf8" });
  assert.equal(result.status, 0);
  return result.stdout.trim().split("\n").map(line => line.trim().split(/\s+/))
    .filter(([, group, state]) => pids.has(Number(group)) && !state.startsWith("Z"));
}
let sequence = 0;
async function launch(script, extra = {}) {
  const name = `fixture-${++sequence}`;
  // The shell records its own PID; exec keeps ownership unambiguous for cleanup.
  const file = join(cwd, `${name}.cjs`), pidfile = join(cwd, `${name}.pid`);
  writeFileSync(file, script);
  const command = `echo $$ > '${pidfile}'; exec '${process.execPath}' '${file}'`;
  const result = await call({ command, mode: "dispatch", background: true, handsFree: { autoExitOnQuiet: false }, ...extra });
  assert(!result.isError, text(result));
  await until(() => { try { return !!readFileSync(pidfile, "utf8").trim(); } catch { return false; } }, "fixture PID");
  const pid = Number(readFileSync(pidfile, "utf8")); pids.add(pid); pidStarts.set(pid, processState(pid).start);
  return { id: result.details.sessionId, pid, result };
}
function checkFrame(message) {
  const wrappers = loaded.extensions.flatMap(e => e.messageRendererWrappers ?? []);
  assert.equal(wrappers.length, 1);
  const component = new pi.CustomMessageComponent({ role: "custom", timestamp: Date.now(), ...message }, undefined, undefined, 1, [], undefined, undefined, wrappers);
  for (const width of [20, 80, 120]) {
    const lines = component.render(width);
    assert(lines.every(line => visibleWidth(line) <= width));
    assert(lines.some(line => stripTerminalSequences(line).startsWith("╭")));
  }
}
const watchdog = setTimeout(() => { console.error("fixture watchdog"); cleanup().finally(() => process.exit(1)); }, 90000);
async function cleanup() {
  try { overlay?.dispose?.(); await emit("session_shutdown", "quit"); } catch {}
  await delay(2200); // package SIGTERM -> SIGKILL grace, only this fixture's processes
  for (const pid of pids) if (alive(pid)) { try { process.kill(pid, "SIGKILL"); } catch {} }
}
try {
  await load();
  // Native bash retains its upstream schema and exit/timeout behavior.
  const bash = pi.createBashTool(cwd);
  assert.deepEqual(Object.keys(bash.parameters.properties).sort(), ["command", "timeout"]);
  assert.match(text(await bash.execute("ok", { command: "printf native-ok" })), /native-ok/);
  const failedBash = await bash.execute("fail", { command: "exit 7" });
  assert.equal(failedBash.isError, true); assert.equal(failedBash.structuredContent.exit_code, 7);
  await assert.rejects(bash.execute("timeout", { command: "sleep 3", timeout: 0.1 }), /timed out/);
  const native = await bash.execute("full", { command: "printf 'native-line\\n%.0s' {1..2200}" });
  assert(native.details.truncation.truncated);
  await emit("tool_result", undefined, { toolName: "bash", ...native });
  assert.equal(entries.length, 1, "native output automatically retained");

  const success = await launch("if (!process.stdin.isTTY || !process.stdout.isTTY) process.exit(99); setTimeout(() => { console.log('quiet-success'); process.exit(0); }, 27000);");
  const failure = await launch("setTimeout(() => { console.log('quiet-failure'); process.exit(7); }, 27000);");
  await delay(25000); // exceeds upstream 15s grace + 8s silence, with real wall clock
  for (const job of [success, failure]) {
    assert(alive(job.pid)); assert(!completion(job.id), "silence is not completion");
    assert.equal((await call({ sessionId: job.id })).details.status, "running");
  }
  assert(widgets.get("bg-sessions").render(100).some(line => line.includes(success.id)));
  for (const [job, code, marker] of [[success, 0, "quiet-success"], [failure, 7, "quiet-failure"]]) {
    await until(() => completion(job.id), `exit notification ${code}`);
    const notice = completion(job.id);
    assert.equal(notice.options.triggerTurn, true); assert.equal(notice.message.display, true);
    assert.equal(notice.message.details.completionReason, "exited"); assert.equal(notice.message.details.exitCode, code);
    assert.match(notice.message.content, new RegExp(marker)); checkFrame(notice.message);
    const query = await call({ sessionId: job.id });
    assert.match(text(query), /Full output:/); // upstream removes active entry, retains log pointer
    assert(!alive(job.pid));
  }
  const logs = readdirSync(join(agent, "cache/interactive-shell")).filter(name => name.endsWith(".log"));
  const log = join(agent, "cache/interactive-shell", logs.find(name => name.includes(success.id)));
  assert.match(readFileSync(log, "utf8"), /quiet-success/);
  await emit("tool_result", undefined, { toolName: "interactive_shell", details: { fullOutputPath: log, truncation: { truncated: true } }, content: [] });
  assert.equal(entries.length, 1, "external logs never automatically collected");
  const resolver = loaded.extensions.flatMap(e => e.toolRenderers ?? [])[0];
  const row = new pi.ToolExecutionComponent("interactive_shell", "shell-row", { sessionId: success.id }, { outputPad: 1 },
    resolver("interactive_shell", () => undefined), tui, cwd);
  row.updateResult(await call({ sessionId: success.id }));
  assert(row.render(80).some(line => stripTerminalSequences(line).startsWith("╭")));
  await tools.get("artifact").execute("register", { action: "register", path: log }, undefined, undefined, ctx);
  assert.equal(entries.length, 2, "explicit log registration succeeds");

  const input = await launch("process.stdin.once('data', d => { console.log('received:' + d.toString().trim()); process.exit(0); });");
  assert(! (await call({ sessionId: input.id, input: "hello", submit: true })).isError);
  await until(() => completion(input.id), "input notification");
  assert.match(completion(input.id).message.content, /received:hello/);

  // A descendant keeps the same process group; stopping must also clean it up.
  const interactive = await launch("require('node:child_process').spawn('sleep', ['60'], { stdio: 'ignore' }); process.stdin.resume(); setInterval(() => {}, 1000);", { background: false, mode: "hands-free" });
  assert(overlay, "native overlay created");
  for (const width of [40, 100]) assert(overlay.render(width).every(line => visibleWidth(line) <= width));
  overlay.handleInput("x");
  assert.equal((await call({ sessionId: interactive.id })).details.status, "user-takeover");
  overlay.handleInput("\x07"); // Ctrl+G resumes monitoring
  assert.equal(overlay.getSessionStatus(), "running");
  overlay.handleInput("\x02"); // Ctrl+B backgrounds the same PTY
  await until(() => !overlay, "background overlay");
  const attachExtension = loaded.extensions.find(e => e.commands.has("attach"));
  const attaching = attachExtension.commands.get("attach").handler(interactive.id, ctx);
  await until(() => !!overlay, "attach overlay");
  assert(overlay.render(100).some(line => line.includes(interactive.id) || line.includes("fixture")));
  overlay.handleInput("\x02"); await attaching;
  const stopped = await call({ sessionId: interactive.id, kill: true });
  assert.equal(stopped.details.status, "killed"); assert.match(text(stopped), /exit is not confirmed/);
  await until(() => !alive(interactive.pid), "stop fixture process", 5000);

  const timeout = await launch("setInterval(() => {}, 1000);", { timeout: 250 });
  await until(() => completion(timeout.id), "timeout notification");
  assert.equal(completion(timeout.id).message.details.completionReason, "timed-out");
  assert.equal(completion(timeout.id).message.details.exitCode, null);

  const reload = await launch("setTimeout(() => { console.log('after-reload'); process.exit(0); }, 2000);");
  await emit("session_shutdown", "reload"); assert(alive(reload.pid));
  await load("reload");
  await until(() => completion(reload.id), "notification after real loader reload");
  assert.equal(completion(reload.id).message.details.exitCode, 0);
  assert.equal(notices.filter(n => n.message.details?.sessionId === reload.id).length, 1);

  for (const reason of ["new", "resume", "fork", "quit"]) {
    const job = await launch("setInterval(() => {}, 1000);");
    await emit("session_shutdown", reason);
    await until(() => !alive(job.pid), `${reason} cleanup`, 5000);
    if (reason !== "quit") await load(reason);
  }
  await until(() => [...pids].every(pid => !alive(pid)) && liveOwnedGroups().length === 0, "all fixture processes and process groups exited", 6000);
  console.log("PASS Linux PTY: quiet exits 0/7, notifications/logs, input/takeover/attach/stop, reload/new/resume/fork/quit, native bash/artifacts, rounded UI; no fixture processes remain");
} catch (error) {
  console.error(error);
  process.exitCode = 1;
} finally {
  clearTimeout(watchdog); await cleanup();
}
// Upstream retains 5-minute history timers; the isolated fixture process owns them.
process.exit(process.exitCode ?? 0);
