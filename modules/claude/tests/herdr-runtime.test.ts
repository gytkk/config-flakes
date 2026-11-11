import { createHash, randomUUID } from "node:crypto";
import { appendFile, chmod, mkdir, mkdtemp, readFile, rename, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "bun:test";

type DesktopSnapshot = {
  version: number;
  session: string;
  model: string;
  updatedAt: number;
  children: Array<{ id: string; name: string; model: string; activity: string; phase: string;
    unavailable: boolean; stoppedAt?: number; endedAt?: number }>;
};

async function atomicJson(path: string, value: unknown): Promise<void> {
  await writeFile(`${path}.tmp`, JSON.stringify(value));
  await rename(`${path}.tmp`, path);
}

async function bounded<T>(promise: Promise<T>): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([promise, new Promise<never>((_, reject) => {
      timer = setTimeout(() => reject(new Error("Watcher did not exit within four seconds")), 4_000);
    })]);
  } finally { clearTimeout(timer); }
}

async function fixture(desktop = false, terminalSocket = true) {
  const temporary = await mkdtemp(join(tmpdir(), "claude-herdr-runtime-"));
  const runtime = join(temporary, "runtime");
  const claudeConfig = join(temporary, "claude-config");
  const transcript = join(temporary, "session-test.jsonl");
  const sessionId = "session-test";
  const pane = "pane-test";
  const socket = join(temporary, "herdr.sock");
  const log = join(temporary, "herdr.log");
  const socketLog = join(temporary, "core-sockets.log");
  const terminal = "remote/terminal-test";
  const coreSocket = terminalSocket ? join(temporary, "terminal.sock") : join(temporary, "fallback.sock");
  const fakeHerdr = join(temporary, "herdr");
  const fakePs = join(temporary, "ps");
  const token = randomUUID();
  const generation = randomUUID();
  const identity = desktop ? `desktop\0${coreSocket}\0${terminal}` : `${socket}\0${pane}`;
  const key = createHash("sha256").update(identity).digest("hex").slice(0, 24);
  const directory = join(runtime, `claude-herdr-subagents-${process.getuid?.() ?? "user"}`, key);
  const ownerPath = join(directory, "owner.json");
  const events = join(directory, generation, "events.jsonl");
  const childPath = join(temporary, sessionId, "subagents", "agent-child-test.jsonl");
  const startedAt = Date.now() - 100;
  const owner = { token, generation, sessionId, transcript, claudePid: process.pid,
    claudeIdentity: "synthetic-process-identity", replayAfter: 0 };
  let watcher: ReturnType<typeof Bun.spawn> | undefined;
  try {
    await mkdir(join(directory, generation), { recursive: true });
    await mkdir(join(temporary, sessionId, "subagents"), { recursive: true });
    await writeFile(log, "");
    await writeFile(fakeHerdr, `#!${process.execPath}\nconst { appendFile } = await import("node:fs/promises");\nawait appendFile(process.env.HERDR_TEST_LOG, JSON.stringify(process.argv.slice(2)) + "\\n");\n`);
    await chmod(fakeHerdr, 0o700);
    await writeFile(join(temporary, "herdr-core"), `#!${process.execPath}\nimport { appendFile } from "node:fs/promises";\nawait appendFile(process.env.HERDR_TEST_LOG, JSON.stringify(process.argv.slice(2)) + "\\n");\nawait appendFile(process.env.HERDR_TEST_SOCKET_LOG, JSON.stringify(process.env.HC_SOCKET) + "\\n");\n`);
    await chmod(join(temporary, "herdr-core"), 0o700);
    await writeFile(socketLog, "");
    await writeFile(fakePs, "#!/bin/sh\nprintf '%s\\n' 'synthetic-process-identity'\n");
    await chmod(fakePs, 0o700);
    await writeFile(transcript, "");
    await writeFile(events, JSON.stringify({ kind: "start", id: "child-test", at: startedAt, name: "reviewer" }) + "\n");
    await writeFile(childPath, JSON.stringify({ sessionId, agentId: "child-test", isSidechain: true,
      type: "assistant", timestamp: new Date(startedAt + 1).toISOString(),
      message: { model: "claude-test-model", content: [{ type: "text", text: "First progress" }] } }) + "\n");
    await atomicJson(ownerPath, owner);
    watcher = Bun.spawn([process.execPath,
      fileURLToPath(new URL("../files/herdr-subagents/index.ts", import.meta.url)), "watch", token], {
      env: { ...process.env, HERDR_ENV: desktop ? "0" : "1", HERDR_SOCKET_PATH: socket, HERDR_PANE_ID: pane,
        HC_TERMINAL_ID: terminal, HC_TERMINAL_SOCKET: terminalSocket ? coreSocket : "",
        HC_SOCKET: terminalSocket ? join(temporary, "wrong.sock") : coreSocket,
        HERDR_BIN_PATH: fakeHerdr, HERDR_TEST_LOG: log, HERDR_TEST_SOCKET_LOG: socketLog, XDG_RUNTIME_DIR: runtime,
        CLAUDE_CONFIG_DIR: claudeConfig,
        PATH: `${temporary}:${process.env.PATH ?? ""}` },
      stdin: "ignore", stdout: "ignore", stderr: "inherit",
    });
  } catch (error) {
    if (watcher) { watcher.kill(); await watcher.exited; }
    await rm(temporary, { recursive: true, force: true });
    throw error;
  }
  async function reports(): Promise<string[][]> {
    return (await readFile(log, "utf8")).split("\n").filter(Boolean).map(line => JSON.parse(line));
  }
  async function waitFor(value: string, timeout = 4_000): Promise<string[][]> {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      const result = await reports();
      if (result.some(args => args.includes(value))) return result;
      await Bun.sleep(25);
    }
    throw new Error(`Timed out waiting for ${value}; reports: ${JSON.stringify(await reports())}`);
  }
  // This fixture observes a separate real watcher process; fake timers cannot drive it.
  async function waitForSnapshot(predicate: (snapshot: DesktopSnapshot) => boolean, timeout = 4_000): Promise<DesktopSnapshot> {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      for (const args of await reports()) {
        if (args[0] !== "label" || !args[2]?.startsWith("claude.subagents={")) continue;
        const snapshot: DesktopSnapshot = JSON.parse(args[2].slice("claude.subagents=".length));
        if (predicate(snapshot)) return snapshot;
      }
      await Bun.sleep(25);
    }
    throw new Error(`Timed out waiting for desktop snapshot; reports: ${JSON.stringify(await reports())}`);
  }
  return { transcript, sessionId, childPath, claudeConfig, events, ownerPath, owner, watcher: watcher!, reports, waitFor, waitForSnapshot,
    terminal, coreSocket, socketLog, fakePs,
    async cleanup() {
      await atomicJson(ownerPath, { ...owner, stopped: true });
      try { await bounded(watcher!.exited); }
      catch { watcher!.kill(); await watcher!.exited; }
      await rm(temporary, { recursive: true, force: true });
    } };
}

test("desktop watcher publishes all lifecycle states, uncertainty and fresh snapshots, then clears its label", async () => {
  const f = await fixture(true);
  try {
    const initial = await f.waitForSnapshot(snapshot => snapshot.children[0]?.model === "claude-test-model");
    expect(initial).toMatchObject({ version: 1, session: f.owner.generation, model: "" });
    expect(initial.children[0]).toMatchObject({ id: "child-test", name: "reviewer",
      activity: "First progress", phase: "running", unavailable: false });
    const at = Date.now();
    await appendFile(f.events, Array.from({ length: 9 }, (_, index) => JSON.stringify({
      kind: "start", id: `extra-${index}`, at, name: "\u001b[31m" + "한".repeat(100),
    })).join("\n") + "\n");
    const expanded = await f.waitForSnapshot(snapshot => snapshot.children.length === 10);
    expect(expanded.children.slice(1).every(child => child.phase === "running" && child.unavailable)).toBeTrue();
    expect(Array.from(expanded.children[1].name)).toHaveLength(80);
    expect(expanded.children[1].name).not.toContain("\u001b");
    await appendFile(f.events, JSON.stringify({ kind: "stop", id: "child-test", at: Date.now(),
      activity: "Handback pending" }) + "\n");
    const stopping = await f.waitForSnapshot(snapshot => snapshot.children[0]?.phase === "stopping");
    expect(stopping.children[0]).toMatchObject({ activity: "Handback pending", unavailable: false });
    expect(stopping.children[0].stoppedAt).toBeNumber();
    expect(stopping.children[0].endedAt).toBeUndefined();
    for (const [id, phase] of [["child-test", "completed"], ["extra-0", "failed"], ["extra-1", "interrupted"]]) {
      await appendFile(f.events, JSON.stringify({ kind: "result", id, phase, at: Date.now(),
        activity: `${phase} result` }) + "\n");
    }
    const settled = await f.waitForSnapshot(snapshot => snapshot.children.some(child => child.phase === "interrupted"));
    expect(settled.children.find(child => child.id === "child-test")).toMatchObject({
      phase: "completed", activity: "completed result", unavailable: false,
    });
    expect(settled.children.find(child => child.id === "extra-0")).toMatchObject({
      phase: "failed", activity: "failed result", unavailable: true,
    });
    expect(settled.children.find(child => child.id === "extra-1")).toMatchObject({
      phase: "interrupted", activity: "interrupted result", unavailable: true,
    });
    const refreshed = await f.waitForSnapshot(snapshot => snapshot.updatedAt >= settled.updatedAt + 5_000, 7_000);
    expect(refreshed.session).toBe(initial.session);
    await f.waitForSnapshot(snapshot => snapshot.children.length === 7, 7_000);
    expect((await f.reports()).every(args => args[0] === "label" && args[1] === f.terminal)).toBeTrue();
    const sockets = (await readFile(f.socketLog, "utf8")).split("\n").filter(Boolean).map(line => JSON.parse(line));
    expect(sockets.every(socket => socket === f.coreSocket)).toBeTrue();
    await atomicJson(f.ownerPath, { ...f.owner, stopped: true });
    expect(await bounded(f.watcher.exited)).toBe(0);
    expect((await f.reports()).at(-1)).toEqual(["label", f.terminal, "claude.subagents="]);
  } finally { await f.cleanup(); }
}, 25_000);

test("desktop watcher falls back to HC_SOCKET and clears metadata when the owning process dies", async () => {
  const f = await fixture(true, false);
  try {
    await f.waitForSnapshot(snapshot => snapshot.children.length === 1);
    expect((await readFile(f.socketLog, "utf8")).trim()).toBe(JSON.stringify(f.coreSocket));
    // Change ps identity without changing the PID, exercising PID-reuse protection.
    await writeFile(f.fakePs, "#!/bin/sh\nprintf '%s\\n' 'replacement-process-identity'\n");
    await f.waitFor("claude.subagents=", 7_000);
    expect(await bounded(f.watcher.exited)).toBe(0);
    expect((await f.reports()).at(-1)).toEqual(["label", f.terminal, "claude.subagents="]);
  } finally { await f.cleanup(); }
}, 12_000);

test("replaced desktop owners cannot be cleared or overwritten by an old watcher", async () => {
  const f = await fixture(true);
  try {
    await f.waitForSnapshot(snapshot => snapshot.children.length === 1);
    const replacement = { ...f.owner, token: randomUUID(), generation: randomUUID(), sessionId: "replacement" };
    await atomicJson(f.ownerPath, replacement);
    await appendFile(f.events, JSON.stringify({ kind: "stop", id: "child-test", at: Date.now() }) + "\n");
    const before = await f.reports();
    expect(await bounded(f.watcher.exited)).toBe(0);
    expect(await f.reports()).toEqual(before);
    expect(JSON.parse(await readFile(f.ownerPath, "utf8"))).toEqual(replacement);
  } finally { await f.cleanup(); }
}, 10_000);

test("watcher follows child progress, waits for confirmed completion, and clears stopped owners", async () => {
  const f = await fixture();
  try {
    await f.waitFor("subagent_1=reviewer / claude-test-model");
    await f.waitFor("subagent_1_activity=|  First progress");
    await f.waitFor("subagent_1_status=●");
    const updateAt = Date.now();
    await appendFile(f.childPath, JSON.stringify({ sessionId: f.sessionId, agentId: "child-test", isSidechain: true,
      type: "assistant", timestamp: new Date(updateAt).toISOString(),
      message: { model: "claude-test-model", content: [{ type: "text", text: "Updated progress" }] } }) + "\n");
    await f.waitFor("subagent_1_activity=|  Updated progress");
    await appendFile(f.events, JSON.stringify({ kind: "stop", id: "child-test", at: updateAt + 1,
      activity: "Handback pending" }) + "\n");
    const pending = await f.waitFor("subagent_1_activity=|  Handback pending");
    expect(pending.some(args => args.includes("subagent_1_status=✓"))).toBeFalse();
    expect(pending.some(args => args.includes("subagent_1_status=○"))).toBeTrue();
    await appendFile(f.transcript, JSON.stringify({ sessionId: f.sessionId, type: "user",
      timestamp: new Date(Date.now()).toISOString(), message: { content: [{ type: "tool_result", tool_use_id: "task-one" }] },
      toolUseResult: { agentId: "child-test", status: "completed", content: [{ type: "text", text: "Confirmed result" }] } }) + "\n");
    await f.waitFor("subagent_1_status=✓");
    await f.waitFor("subagent_1_activity=|  Confirmed result");
    await atomicJson(f.ownerPath, { ...f.owner, stopped: true });
    expect(await bounded(f.watcher.exited)).toBe(0);
    const reports = await f.reports();
    expect(reports.at(-2)).toContain("--clear-token");
    expect(reports.at(-2)).toContain("subagent_1");
    expect(reports.slice(-2).every(args => !args.includes("--token"))).toBeTrue();
    for (const args of reports) {
      expect(args.filter(arg => arg === "--token" || arg === "--clear-token").length).toBeLessThanOrEqual(15);
      expect(args.some(arg => arg.includes("claude_model"))).toBeFalse();
      expect(args).toContain("15000");
    }
  } finally { await f.cleanup(); }
}, 15_000);

test("watcher confirms a stopped teammate's interruption after removal from its owning team", async () => {
  const f = await fixture();
  try {
    const configPath = join(f.claudeConfig, "teams", "team-test", "config.json");
    await mkdir(join(f.claudeConfig, "teams", "team-test"), { recursive: true });
    await atomicJson(f.childPath.replace(/\.jsonl$/, ".meta.json"), { name: "reviewer", teamName: "team-test" });
    const team = { name: "team-test", leadSessionId: f.sessionId,
      members: [{ agentId: "team-lead@team-test" }, { agentId: "reviewer@team-test" }] };
    await atomicJson(configPath, team);
    await f.waitFor("subagent_1_status=●");
    await appendFile(f.events, JSON.stringify({ kind: "stop", id: "child-test", at: Date.now(),
      activity: "Shutdown accepted" }) + "\n");
    await f.waitFor("subagent_1_status=○");
    await Bun.sleep(750);
    const present = await f.reports();
    expect(present.some(args => args.includes("subagent_1_status=■"))).toBeFalse();
    expect(present.some(args => args.includes("subagent_1"))).toBeFalse();

    await atomicJson(configPath, { ...team, members: team.members.slice(0, 1) });
    await f.waitFor("subagent_1_status=■");
    const cleared = await f.waitFor("subagent_1", 12_000);
    const latestChildReport = cleared.findLast(args => args.some(arg => arg === "subagent_1" || arg.startsWith("subagent_1=")))!;
    expect(latestChildReport).toContain("--clear-token");
    expect(latestChildReport).toContain("subagent_1");
    expect(latestChildReport.some(arg => arg.startsWith("subagent_1="))).toBeFalse();
  } finally { await f.cleanup(); }
}, 20_000);

test("watcher clears stop-only children after ten seconds without new events and shows resumed activity", async () => {
  const f = await fixture();
  try {
    await f.waitFor("subagent_1_status=●");
    const stoppedAt = Date.now();
    await appendFile(f.events, JSON.stringify({ kind: "stop", id: "child-test", at: stoppedAt }) + "\n");
    await f.waitFor("subagent_1_status=○");
    const cleared = await f.waitFor("subagent_1", 12_000);
    expect(Date.now() - stoppedAt).toBeGreaterThanOrEqual(10_000);
    const clearedRow = cleared.findLast(args => args.includes("subagent_1"));
    expect(clearedRow).toContain("--clear-token");
    expect(clearedRow).toContain("subagent_1_activity");
    await appendFile(f.childPath, JSON.stringify({ sessionId: f.sessionId, agentId: "child-test", isSidechain: true,
      type: "assistant", timestamp: new Date().toISOString(),
      message: { model: "claude-test-model", content: [{ type: "text", text: "Resumed progress" }] } }) + "\n");
    const resumed = await f.waitFor("subagent_1_activity=|  Resumed progress");
    expect(resumed.findLast(args => args.includes("subagent_1_activity=|  Resumed progress"))).toContain("subagent_1_status=●");
  } finally { await f.cleanup(); }
}, 20_000);

test("a replaced owner's watcher exits without clearing replacement metadata", async () => {
  const f = await fixture();
  try {
    await f.waitFor("subagent_1_status=●");
    await f.waitFor("subagent_7_activity");
    const before = await f.reports();
    await atomicJson(f.ownerPath, { ...f.owner, token: randomUUID(), generation: randomUUID() });
    expect(await bounded(f.watcher.exited)).toBe(0);
    expect(await f.reports()).toEqual(before);
  } finally { await f.cleanup(); }
}, 10_000);

for (const desktop of [false, true]) {
test(`${desktop ? "Desktop" : "TUI"} hook commands reuse one watcher and only the owning session can stop it`, async () => {
  const temporary = await mkdtemp(join(tmpdir(), "claude-herdr-hooks-"));
  const runtime = join(temporary, "runtime");
  const sessionId = "hook-session";
  const transcript = join(temporary, `${sessionId}.jsonl`);
  const pane = "hook-pane";
  const socket = join(temporary, "herdr.sock");
  const log = join(temporary, "herdr.log");
  const terminal = "remote/hook-terminal";
  const fakeHerdr = join(temporary, desktop ? "herdr-core" : "herdr");
  const identity = desktop ? `desktop\0${socket}\0${terminal}` : `${socket}\0${pane}`;
  const key = createHash("sha256").update(identity).digest("hex").slice(0, 24);
  const ownerPath = join(runtime, `claude-herdr-subagents-${process.getuid?.() ?? "user"}`, key, "owner.json");
  const env = { ...process.env, HERDR_ENV: desktop ? "0" : "1", HERDR_SOCKET_PATH: socket, HERDR_PANE_ID: pane,
    HC_TERMINAL_ID: terminal, HC_TERMINAL_SOCKET: socket, HC_SOCKET: join(temporary, "wrong.sock"),
    HERDR_BIN_PATH: fakeHerdr, HERDR_TEST_LOG: log, XDG_RUNTIME_DIR: runtime,
    PATH: `${temporary}:${process.env.PATH ?? ""}` };
  let workerPid: number | undefined;
  const alive = (pid: number): boolean => {
    try { process.kill(pid, 0); return true; }
    catch (error: any) { if (error.code === "ESRCH") return false; throw error; }
  };
  async function readOwner(): Promise<Record<string, any>> {
    return JSON.parse(await readFile(ownerPath, "utf8"));
  }
  async function hook(hook_event_name: string, extra: Record<string, unknown> = {}, identity = "synthetic-process-identity"): Promise<void> {
    // Bun's test runner returned empty subprocess input with Response and pipe stdin.
    const input = join(temporary, "hook-input.json");
    await writeFile(input, JSON.stringify({ hook_event_name, session_id: sessionId, transcript_path: transcript, ...extra }));
    const command = Bun.spawn([process.execPath,
      fileURLToPath(new URL("../files/herdr-subagents/index.ts", import.meta.url))], {
      env: { ...env, HERDR_TEST_PROCESS_IDENTITY: identity }, stdin: Bun.file(input), stdout: "pipe", stderr: "pipe",
    });
    const stderr = await new Response(command.stderr).text();
    expect(await bounded(command.exited)).toBe(0);
    expect(stderr).toBe("");
  }
  async function waitForReport(value: string | ((args: string[]) => boolean)): Promise<void> {
    const deadline = Date.now() + 4_000;
    while (Date.now() < deadline) {
      const lines = (await readFile(log, "utf8")).split("\n").filter(Boolean);
      if (lines.some(line => {
        const args: string[] = JSON.parse(line);
        return typeof value === "string" ? args.includes(value) : value(args);
      })) return;
      await Bun.sleep(25);
    }
    throw new Error(`Timed out waiting for hook watcher report ${value}: ${await readFile(log, "utf8")}`);
  }
  async function waitForExit(pid: number): Promise<void> {
    const deadline = Date.now() + 4_000;
    while (Date.now() < deadline) {
      if (!alive(pid)) return;
      await Bun.sleep(25);
    }
    throw new Error(`Detached watcher ${pid} did not exit`);
  }
  try {
    await mkdir(runtime, { recursive: true });
    await mkdir(join(temporary, sessionId, "subagents"), { recursive: true });
    await writeFile(log, "");
    await writeFile(transcript, "");
    await writeFile(fakeHerdr, `#!${process.execPath}\nconst { appendFile } = await import("node:fs/promises");\nawait appendFile(process.env.HERDR_TEST_LOG, JSON.stringify(process.argv.slice(2)) + "\\n");\n`);
    await chmod(fakeHerdr, 0o700);
    const fakePs = join(temporary, "ps");
    await writeFile(fakePs, `#!/bin/sh\ncase "$*" in\n  *ppid=*) printf '%s\\n' '${process.pid} claude' ;;\n  *) printf '%s\\n' "$HERDR_TEST_PROCESS_IDENTITY" ;;\nesac\n`);
    await chmod(fakePs, 0o700);
    await hook("SessionStart");
    const initial = await readOwner();
    workerPid = initial.workerPid;
    expect(typeof workerPid).toBe("number");
    expect(alive(workerPid!)).toBeTrue();
    await hook("SessionStart");
    expect((await readOwner()).workerPid).toBe(workerPid);
    expect((await readOwner()).token).toBe(initial.token);
    await hook("SubagentStart", { agent_id: "hook-child", agent_type: "Explore" });
    expect((await readOwner()).workerPid).toBe(workerPid);
    await appendFile(join(temporary, sessionId, "subagents", "agent-hook-child.jsonl"),
      JSON.stringify({ sessionId, agentId: "hook-child", isSidechain: true, type: "assistant",
        timestamp: new Date().toISOString(), message: { model: "hook-model",
          content: [{ type: "text", text: "Hook child progress" }] } }) + "\n");
    if (desktop) {
      await waitForReport(args => {
        if (!args[2]?.startsWith("claude.subagents={")) return false;
        const snapshot: DesktopSnapshot = JSON.parse(args[2].slice("claude.subagents=".length));
        return snapshot.session === initial.generation && snapshot.children.some(child => child.id === "hook-child"
          && child.model === "hook-model" && child.activity === "Hook child progress" && !child.unavailable);
      });
    } else {
      await waitForReport("subagent_1=Explore (hook-c) / hook-model");
      await waitForReport("subagent_1_status=●");
    }
    await hook("SubagentStop", { session_id: "unrelated-session", agent_id: "hook-child" });
    await hook("SessionEnd", { session_id: "unrelated-session" });
    expect(await readOwner()).toEqual(initial);
    expect(alive(workerPid!)).toBeTrue();
    await hook("SessionEnd", {}, "old-process-identity");
    expect(await readOwner()).toEqual(initial);
    await writeFile(log, "");
    await hook("SessionEnd");
    expect((await readOwner()).stopped).toBeTrue();
    if (desktop) await waitForReport("claude.subagents=");
    else {
      await waitForReport("subagent_1");
      await waitForReport("subagent_7_activity");
    }
    await waitForExit(workerPid!);
    const finalReports = (await readFile(log, "utf8")).split("\n").filter(Boolean).map(line => JSON.parse(line) as string[]);
    expect(finalReports.slice(-2).every(args => !args.includes("--token"))).toBeTrue();
    expect(finalReports.every(args => args[0] === (desktop ? "label" : "pane"))).toBeTrue();
    if (desktop) {
      expect(finalReports.at(-1)).toEqual(["label", terminal, "claude.subagents="]);
      const replacementSession = "replacement-session";
      const replacementTranscript = join(temporary, `${replacementSession}.jsonl`);
      const replacementHook = { session_id: replacementSession, transcript_path: replacementTranscript };
      await writeFile(replacementTranscript, "");
      await hook("SessionStart", replacementHook);
      const replacement = await readOwner();
      workerPid = replacement.workerPid;
      expect(replacement.generation).not.toBe(initial.generation);
      expect(replacement.token).not.toBe(initial.token);
      await hook("SubagentStart", { ...replacementHook, agent_id: "replacement-child", agent_type: "Explore" });
      await waitForReport(args => {
        if (!args[2]?.startsWith("claude.subagents={")) return false;
        const snapshot: DesktopSnapshot = JSON.parse(args[2].slice("claude.subagents=".length));
        return snapshot.session === replacement.generation && snapshot.children.some(child => child.id === "replacement-child");
      });
      await hook("SessionEnd");
      expect(await readOwner()).toEqual(replacement);
      expect(alive(workerPid!)).toBeTrue();
      await hook("SessionEnd", replacementHook);
      expect((await readOwner()).stopped).toBeTrue();
      await waitForExit(workerPid!);
      const latestReports: string[][] = (await readFile(log, "utf8")).split("\n").filter(Boolean).map(line => JSON.parse(line));
      expect(latestReports.at(-1)).toEqual(["label", terminal, "claude.subagents="]);
    }
  } finally {
    const current = await readOwner().catch(() => undefined);
    if (current) await atomicJson(ownerPath, { ...current, stopped: true });
    const cleanupPid = workerPid ?? current?.workerPid;
    if (typeof cleanupPid === "number") {
      try { await waitForExit(cleanupPid); }
      catch { if (alive(cleanupPid)) process.kill(cleanupPid, "SIGKILL"); await waitForExit(cleanupPid); }
    }
    await rm(temporary, { recursive: true, force: true });
  }
}, 20_000);
}
