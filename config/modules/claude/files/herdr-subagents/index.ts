import { createHash, randomUUID } from "node:crypto";
import { execFile, spawn } from "node:child_process";
import { constants } from "node:fs";
import { access, appendFile, mkdir, open, readFile, rename, rm, stat, writeFile } from "node:fs/promises";
import { homedir, tmpdir } from "node:os";
import { basename, dirname, isAbsolute, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { JsonlTail } from "../../../herdr/files/jsonl-tail";
import { childTranscripts, teammateRemoved } from "./io";
import { applyEvent, consumeChild, consumeParent, hookEvent, object, sidebarTokens, validId, type Child, type Event } from "./projection";

const exec = promisify(execFile);
const SCRIPT = fileURLToPath(import.meta.url);
const SOURCE = "claude:herdr-subagents";
const TTL_MS = 15_000;
const REFRESH_MS = 5_000;
const PROBE_MS = 5_000;
const LOCK_WAIT_MS = 5_000;
const LOCK_STALE_MS = 10_000;

type Owner = {
  token: string;
  sessionId: string;
  transcript: string;
  claudePid: number;
  claudeIdentity: string;
  replayAfter: number;
  generation: string;
  workerPid?: number;
  workerIdentity?: string;
  stopped?: boolean;
};

async function json(path: string): Promise<any | undefined> {
  try { return JSON.parse(await readFile(path, "utf8")); }
  catch (error: any) {
    if (error.code === "ENOENT") return undefined;
    throw new Error(`Reading ${path}: ${error.message}`);
  }
}

async function atomicJson(path: string, value: unknown): Promise<void> {
  const temporary = `${path}.${randomUUID()}.tmp`;
  await writeFile(temporary, JSON.stringify(value), { mode: 0o600 });
  await rename(temporary, path);
}

function code(error: unknown): string | undefined {
  return typeof error === "object" && error !== null && "code" in error
    ? String((error as NodeJS.ErrnoException).code)
    : undefined;
}

async function withOwnerLock<T>(directory: string, action: () => Promise<T>): Promise<T> {
  const lock = join(directory, "owner.lock");
  const holderPath = join(lock, "holder.json");
  const holder = { pid: process.pid, token: randomUUID() };
  const deadline = Date.now() + LOCK_WAIT_MS;
  while (true) {
    try {
      await mkdir(lock, { mode: 0o700 });
      await writeFile(holderPath, JSON.stringify(holder), { mode: 0o600 });
      break;
    } catch (error) {
      if (code(error) !== "EEXIST") throw error;
      try {
        if (Date.now() - (await stat(lock)).mtimeMs > LOCK_STALE_MS) {
          const current = await json(holderPath);
          if (!current || typeof current.pid !== "number" || !alive(current.pid)) {
            await rm(lock, { recursive: true, force: true });
            continue;
          }
        }
      } catch (inspectionError) {
        if (code(inspectionError) !== "ENOENT") throw inspectionError;
        continue;
      }
      if (Date.now() >= deadline) throw new Error("Timed out waiting for the sidebar owner lock");
      await Bun.sleep(25);
    }
  }
  try { return await action(); }
  finally {
    const current = await json(holderPath);
    if (current?.pid === holder.pid && current?.token === holder.token) {
      await rm(lock, { recursive: true, force: true });
    }
  }
}

function alive(pid: number | undefined): boolean {
  if (!pid || pid < 2) return false;
  try { process.kill(pid, 0); return true; }
  catch (error: any) {
    if (error.code === "ESRCH") return false;
    throw error;
  }
}

async function processIdentity(pid: number): Promise<string | undefined> {
  try {
    const { stdout } = await exec("ps", ["-p", String(pid), "-o", "lstart=", "-o", "comm="], {
      timeout: 1_000,
      env: { ...process.env, LC_ALL: "C" },
    });
    return stdout.trim() || undefined;
  } catch { return undefined; }
}

async function claudeAncestor(): Promise<{ pid: number; identity: string }> {
  let pid = process.ppid;
  for (let depth = 0; depth < 12 && pid > 1; depth++) {
    const { stdout } = await exec("ps", ["-p", String(pid), "-o", "ppid=", "-o", "comm="], {
      timeout: 1_000,
      env: { ...process.env, LC_ALL: "C" },
    });
    const match = stdout.trim().match(/^(\d+)\s+(.+)$/);
    if (!match) break;
    if (/^\.?claude(?:-.*)?$/.test(basename(match[2]))) {
      const identity = await processIdentity(pid);
      if (identity) return { pid, identity };
      break;
    }
    pid = Number(match[1]);
  }
  throw new Error("Cannot identify the owning Claude process; sidebar watcher was not started");
}

async function sameProcess(pid: number, identity: string): Promise<boolean> {
  return alive(pid) && await processIdentity(pid) === identity;
}

function processStartedAt(identity: string): number {
  const parsed = Date.parse(identity.split(/\s+/).slice(0, 5).join(" "));
  return Number.isFinite(parsed) ? parsed : Date.now();
}

async function environment(): Promise<{ pane: string; binary: string; directory: string } | undefined> {
  const pane = process.env.HERDR_PANE_ID;
  const socket = process.env.HERDR_SOCKET_PATH;
  if (process.env.HERDR_ENV !== "1" || !pane || !socket) return;
  let runtimeDirectory = tmpdir();
  const configured = process.env.XDG_RUNTIME_DIR;
  if (configured && isAbsolute(configured)) {
    try {
      if ((await stat(configured)).isDirectory()) {
        await access(configured, constants.W_OK | constants.X_OK);
        runtimeDirectory = configured;
      }
    } catch (error) {
      if (!["ENOENT", "ENOTDIR", "EACCES", "EPERM", "EROFS", "ELOOP", "ENAMETOOLONG"].includes(code(error) ?? "")) throw error;
    }
  }
  const key = createHash("sha256").update(`${socket}\0${pane}`).digest("hex").slice(0, 24);
  return {
    pane,
    binary: process.env.HERDR_BIN_PATH || "herdr",
    directory: join(runtimeDirectory, `claude-herdr-subagents-${process.getuid?.() ?? "user"}`, key),
  };
}

export async function handleHook(payload: unknown): Promise<void> {
  const env = await environment();
  const hook = object(payload);
  if (!env || !hook || !validId(hook.session_id)) return;
  const event = hookEvent(hook);
  const sessionEvent = ["SessionStart", "SessionEnd"].includes(hook.hook_event_name);
  if ((!event && !sessionEvent) || (sessionEvent && hook.agent_id)) return;
  await mkdir(env.directory, { recursive: true, mode: 0o700 });
  const ownerPath = join(env.directory, "owner.json");
  await withOwnerLock(env.directory, async () => {
    let owner: Owner | undefined = await json(ownerPath);
    if (hook.hook_event_name === "SessionEnd") {
      if (owner?.sessionId !== hook.session_id) return;
      const claude = await claudeAncestor();
      if (owner.claudePid === claude.pid && owner.claudeIdentity === claude.identity) await atomicJson(ownerPath, { ...owner, stopped: true });
      return;
    }
    if (owner && owner.sessionId !== hook.session_id && hook.hook_event_name !== "SessionStart") return;
    if (owner?.sessionId === hook.session_id && owner.stopped && hook.hook_event_name !== "SessionStart") return;
    const transcript = typeof hook.transcript_path === "string" ? resolve(hook.transcript_path) : undefined;
    // A subagent or another session must not take ownership of the parent's pane.
    if (!transcript || basename(transcript) !== `${hook.session_id}.jsonl` || basename(dirname(transcript)) === "subagents") return;
    const claude = await claudeAncestor();
    if (owner && hook.hook_event_name !== "SessionStart" && (owner.claudePid !== claude.pid || owner.claudeIdentity !== claude.identity)) return;
    const sameSession = owner?.sessionId === hook.session_id && !owner.stopped
      && owner.claudePid === claude.pid && owner.claudeIdentity === claude.identity;
    const healthy = sameSession && typeof owner?.workerPid === "number" && typeof owner.workerIdentity === "string"
      && await sameProcess(owner.workerPid, owner.workerIdentity);
    if (!healthy) {
      owner = {
        token: randomUUID(), sessionId: hook.session_id, transcript,
        claudePid: claude.pid, claudeIdentity: claude.identity,
        replayAfter: sameSession ? owner!.replayAfter : processStartedAt(claude.identity) - 1_000,
        generation: sameSession ? owner!.generation : randomUUID(),
      };
      await mkdir(join(env.directory, owner.generation), { recursive: true, mode: 0o700 });
      await atomicJson(ownerPath, owner);
    }
    if (!owner) return;
    if (event) {
      await appendFile(join(env.directory, owner.generation, "events.jsonl"), JSON.stringify(event) + "\n", { mode: 0o600 });
    }
    if (healthy) return;
    const log = await open(join(env.directory, "watcher.log"), "a", 0o600);
    try {
      const worker = spawn(process.execPath, [SCRIPT, "watch", owner.token], {
        detached: true, stdio: ["ignore", "ignore", log.fd], env: process.env,
      });
      await new Promise<void>((accept, reject) => { worker.once("spawn", accept); worker.once("error", reject); });
      owner.workerPid = worker.pid;
      owner.workerIdentity = await processIdentity(worker.pid!);
      await atomicJson(ownerPath, owner);
      worker.unref();
    } finally { await log.close(); }
  });
}

export async function watch(token: string): Promise<void> {
  const env = await environment();
  if (!env) return;
  const ownerPath = join(env.directory, "owner.json");
  const initial: Owner | undefined = await json(ownerPath);
  if (!initial || initial.token !== token) return;
  const parent = new JsonlTail();
  const events = new JsonlTail();
  const children = new Map<string, Child>();
  const tails = new Map<string, { reader: JsonlTail; verified: boolean; tainted: boolean; startedAt: number }>();
  const tasks = new Map<string, string>();
  const childRoot = join(dirname(initial.transcript), initial.sessionId, "subagents");
  const teamsRoot = join(resolve(process.env.CLAUDE_CONFIG_DIR || join(homedir(), ".claude")), "teams");
  let paths = new Map<string, string>();
  const errors = new Map<string, string>();
  let published = "";
  let lastPublished = 0;
  let nextProbe = 0;
  let nextScan = 0;
  let unavailableSince = 0;
  const report = async (states: Child[]) => {
    const tokens = sidebarTokens(states);
    const serialized = JSON.stringify(tokens);
    if (serialized === published && (Object.values(tokens).every((value) => value === null) || Date.now() - lastPublished < REFRESH_MS)) return;
    await withOwnerLock(env.directory, async () => {
      if ((await json(ownerPath))?.token !== token) return;
      const entries = Object.entries(tokens);
      // Herdr accepts 16 keys per request; batches of 15 preserve child triplets.
      for (let offset = 0; offset < entries.length; offset += 15) {
        const args = ["pane", "report-metadata", env.pane, "--source", SOURCE, "--agent", "claude", "--ttl-ms", String(TTL_MS)];
        for (const [key, value] of entries.slice(offset, offset + 15)) args.push(...(value === null ? ["--clear-token", key] : ["--token", `${key}=${value}`]));
        await exec(env.binary, args, { timeout: 2_000 }).catch(() => { throw new Error("Herdr metadata update failed"); });
      }
      published = serialized;
      lastPublished = Date.now();
    });
  };
  const diagnose = (key: string, error: unknown) => {
    const message = error instanceof Error ? error.message : String(error);
    if (errors.get(key) !== message) console.error(`Claude Herdr sidebar (${key}): ${message}`);
    errors.set(key, message);
  };
  try {
    while (true) {
      const owner: Owner | undefined = await json(ownerPath);
      if (!owner || owner.token !== token) return;
      if (owner.stopped) break;
      if (Date.now() >= nextProbe) {
        if (!await sameProcess(initial.claudePid, initial.claudeIdentity)) break;
        nextProbe = Date.now() + PROBE_MS;
      }
      try {
        if (Date.now() >= nextScan) { paths = await childTranscripts(childRoot); nextScan = Date.now() + PROBE_MS; }
        for (const value of await events.read(join(env.directory, initial.generation, "events.jsonl"))) {
          const event = value as Event;
          if (validId(event?.id) && ["start", "stop", "result"].includes(event.kind)) applyEvent(children, event);
        }
        for (const value of await parent.read(initial.transcript)) consumeParent(children, value, initial.sessionId, initial.replayAfter, tasks);
        for (const child of children.values()) {
          let state = tails.get(child.id);
          if (!state) { state = { reader: new JsonlTail(), verified: false, tainted: false, startedAt: child.startedAt }; tails.set(child.id, state); }
          const tail = state.reader;
          if (state.startedAt !== child.startedAt) { state.tainted = false; state.startedAt = child.startedAt; }
          try {
            const supplied = child.transcript && resolve(child.transcript);
            const within = supplied && relative(childRoot, supplied);
            const safe = supplied && within && !within.startsWith("..") && !isAbsolute(within) && basename(supplied) === `agent-${child.id}.jsonl`;
            const path = safe ? supplied : paths.get(child.id) ?? join(childRoot, `agent-${child.id}.jsonl`);
            const records = await tail.read(path);
            if (tail.reset || !tail.available) state.verified = false;
            for (const value of records) {
              const record = object(value);
              if (record?.agentId && (record.agentId !== child.id || record.sessionId !== initial.sessionId)) { state.tainted = true; break; }
              if (consumeChild(child, value, initial.sessionId)) state.verified = true;
            }
            state.tainted ||= tail.hasGaps;
            child.unavailable = !state.verified || state.tainted || !tail.available || !tail.caughtUp || !parent.available || !parent.caughtUp;
            // A team member can be removed without a terminal Agent result.
            if (child.phase === "stopping" && !child.unavailable && await teammateRemoved(path, teamsRoot, initial.sessionId)) {
              applyEvent(children, { kind: "result", id: child.id, at: Date.now(), phase: "interrupted" });
            }
            errors.delete(child.id);
          } catch (error) { child.unavailable = true; diagnose(child.id, error); }
        }
        errors.delete("read");
      } catch (error) {
        diagnose("read", error);
        for (const child of children.values()) child.unavailable = true;
      }
      try {
        await report([...children.values()]);
        unavailableSince = 0;
        errors.delete("report");
      } catch (error) {
        diagnose("report", error);
        unavailableSince ||= Date.now();
        if (Date.now() - unavailableSince >= TTL_MS) break;
      }
      await Bun.sleep(500);
    }
  } finally {
    await report([]);
  }
}

if (import.meta.main) {
  try {
    if (process.argv[2] === "watch") await watch(process.argv[3]);
    else await handleHook(JSON.parse(await Bun.stdin.text()));
  } catch (error) {
    console.error(`Claude Herdr sidebar: ${error instanceof Error ? error.message : String(error)}`);
    if (process.argv[2] === "watch") process.exitCode = 1;
  }
}
