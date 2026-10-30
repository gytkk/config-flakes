import { createHash, randomUUID } from "node:crypto";
import { appendFile, chmod, mkdir, mkdtemp, readFile, rename, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "bun:test";

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

async function fixture() {
  const temporary = await mkdtemp(join(tmpdir(), "claude-herdr-runtime-"));
  const runtime = join(temporary, "runtime");
  const claudeConfig = join(temporary, "claude-config");
  const transcript = join(temporary, "session-test.jsonl");
  const sessionId = "session-test";
  const pane = "pane-test";
  const socket = join(temporary, "herdr.sock");
  const log = join(temporary, "herdr.log");
  const fakeHerdr = join(temporary, "herdr");
  const fakePs = join(temporary, "ps");
  const token = randomUUID();
  const generation = randomUUID();
  const key = createHash("sha256").update(`${socket}\0${pane}`).digest("hex").slice(0, 24);
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
      env: { ...process.env, HERDR_ENV: "1", HERDR_SOCKET_PATH: socket, HERDR_PANE_ID: pane,
        HERDR_BIN_PATH: fakeHerdr, HERDR_TEST_LOG: log, XDG_RUNTIME_DIR: runtime,
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
  return { transcript, sessionId, childPath, claudeConfig, events, ownerPath, owner, watcher: watcher!, reports, waitFor,
    async cleanup() {
      await atomicJson(ownerPath, { ...owner, stopped: true });
      try { await bounded(watcher!.exited); }
      catch { watcher!.kill(); await watcher!.exited; }
      await rm(temporary, { recursive: true, force: true });
    } };
}

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

test("hook commands reuse one watcher and only the owning session can stop it", async () => {
  const temporary = await mkdtemp(join(tmpdir(), "claude-herdr-hooks-"));
  const runtime = join(temporary, "runtime");
  const sessionId = "hook-session";
  const transcript = join(temporary, `${sessionId}.jsonl`);
  const pane = "hook-pane";
  const socket = join(temporary, "herdr.sock");
  const log = join(temporary, "herdr.log");
  const fakeHerdr = join(temporary, "herdr");
  const key = createHash("sha256").update(`${socket}\0${pane}`).digest("hex").slice(0, 24);
  const ownerPath = join(runtime, `claude-herdr-subagents-${process.getuid?.() ?? "user"}`, key, "owner.json");
  const env = { ...process.env, HERDR_ENV: "1", HERDR_SOCKET_PATH: socket, HERDR_PANE_ID: pane,
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
  async function waitForReport(value: string): Promise<void> {
    const deadline = Date.now() + 4_000;
    while (Date.now() < deadline) {
      const lines = (await readFile(log, "utf8")).split("\n").filter(Boolean);
      if (lines.some(line => (JSON.parse(line) as string[]).includes(value))) return;
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
    await waitForReport("subagent_1=Explore (hook-c) / hook-model");
    await waitForReport("subagent_1_status=●");
    await hook("SubagentStop", { session_id: "unrelated-session", agent_id: "hook-child" });
    await hook("SessionEnd", { session_id: "unrelated-session" });
    expect(await readOwner()).toEqual(initial);
    expect(alive(workerPid!)).toBeTrue();
    await hook("SessionEnd", {}, "old-process-identity");
    expect(await readOwner()).toEqual(initial);
    await writeFile(log, "");
    await hook("SessionEnd");
    expect((await readOwner()).stopped).toBeTrue();
    await waitForReport("subagent_1");
    await waitForReport("subagent_7_activity");
    await waitForExit(workerPid!);
    const finalReports = (await readFile(log, "utf8")).split("\n").filter(Boolean).map(line => JSON.parse(line) as string[]);
    expect(finalReports.slice(-2).every(args => !args.includes("--token"))).toBeTrue();
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
