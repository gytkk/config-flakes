import { createHash, randomUUID } from "node:crypto";
import { execFile, spawn } from "node:child_process";
import { constants } from "node:fs";
import { access, mkdir, open, readFile, readdir, rename, rm, stat, writeFile } from "node:fs/promises";
import { homedir, tmpdir } from "node:os";
import { basename, dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { JsonlTail } from "../../../herdr/files/jsonl-tail";
import { findRollout } from "./io";
import { consumeChild, discoveredChild, displayText, object, sidebarTokens, type ChildState } from "./projection";

const exec = promisify(execFile);
const SCRIPT = fileURLToPath(import.meta.url);
const SOURCE = "codex:herdr-subagents";
const TTL_MS = 15_000;
const REFRESH_MS = 5_000;
const PROBE_MS = 5_000;
const LOCK_WAIT_MS = 5_000;
const LOCK_STALE_MS = 10_000;
const ID = /^[a-zA-Z0-9_-]{1,128}$/;

type Owner = {
  token: string;
  sessionId: string;
  transcript: string;
  codexPid: number;
  codexIdentity: string;
  replayAfter: number;
  workerPid?: number;
  workerIdentity?: string;
  stopped?: boolean;
};

type Registration = { id: string; name: string };

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

async function codexAncestor(): Promise<{ pid: number; identity: string }> {
  let pid = process.ppid;
  for (let depth = 0; depth < 12 && pid > 1; depth++) {
    const { stdout } = await exec("ps", ["-p", String(pid), "-o", "ppid=", "-o", "comm="], {
      timeout: 1_000,
      env: { ...process.env, LC_ALL: "C" },
    });
    const match = stdout.trim().match(/^(\d+)\s+(.+)$/);
    if (!match) break;
    if (/^\.?codex(?:-.*)?$/.test(basename(match[2]))) {
      const { stdout: args } = await exec("ps", ["-p", String(pid), "-o", "args="], { timeout: 1_000 });
      const appServer = [match[2], basename(match[2])].some((executable) => args.trim().startsWith(`${executable} app-server `));
      if (appServer && /(?:^|\s)--managed-daemon(?:\s|$)/.test(args)) {
        throw new Error("Shared Codex daemon has no pane-local owner; restart Codex with --no-daemon");
      }
      const identity = await processIdentity(pid);
      if (identity) return { pid, identity };
      break;
    }
    pid = Number(match[1]);
  }
  throw new Error("Cannot identify the owning Codex process; sidebar watcher was not started");
}

async function sameProcess(pid: number, identity: string): Promise<boolean> {
  return alive(pid) && await processIdentity(pid) === identity;
}

async function rootTranscript(path: string, sessionId: string): Promise<boolean> {
  try {
    const tail = new JsonlTail();
    let records: unknown[] = [];
    do records = await tail.read(path); while (records.length === 0 && !tail.caughtUp);
    const record = object(records[0]);
    const payload = object(record?.payload);
    return record?.type === "session_meta" && payload?.id === sessionId
      && typeof payload?.parent_thread_id !== "string" && typeof object(payload?.source)?.subagent !== "object";
  } catch { return false; }
}

function processStartedAt(identity: string): number {
  const parsed = Date.parse(identity.split(/\s+/).slice(0, 5).join(" "));
  return Number.isFinite(parsed) ? parsed : Date.now();
}

async function environment(): Promise<{ pane: string; binary: string; directory: string; sessions: string } | undefined> {
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
    directory: join(runtimeDirectory, `codex-herdr-subagents-${process.getuid?.() ?? "user"}`, key),
    sessions: join(process.env.CODEX_HOME || join(homedir(), ".codex"), "sessions"),
  };
}

export async function handleHook(payload: unknown): Promise<void> {
  const env = await environment();
  const hook = object(payload);
  if (!env || !hook || typeof hook.session_id !== "string" || !ID.test(hook.session_id)) return;
  const inherited = process.env.CODEX_THREAD_ID;
  if (inherited && inherited !== hook.session_id) return;
  if (!["SessionStart", "SessionEnd", "SubagentStart", "SubagentStop"].includes(hook.hook_event_name)) return;
  const codex = await codexAncestor();
  await mkdir(env.directory, { recursive: true, mode: 0o700 });
  const ownerPath = join(env.directory, "owner.json");
  await withOwnerLock(env.directory, async () => {
    let owner: Owner | undefined = await json(ownerPath);
    if (hook.hook_event_name === "SessionEnd") {
      if (owner?.sessionId === hook.session_id) await atomicJson(ownerPath, { ...owner, stopped: true });
      return;
    }

    const transcript = typeof hook.transcript_path === "string" && hook.transcript_path
      ? resolve(hook.transcript_path)
      : owner?.sessionId === hook.session_id ? owner.transcript : undefined;
    if (!transcript) return;
    if (owner?.sessionId === hook.session_id && owner.stopped && hook.hook_event_name !== "SessionStart") return;
    if (hook.hook_event_name === "SessionStart" && !await rootTranscript(transcript, hook.session_id)) return;
    const existingWorkerHealthy = !!owner && !owner.stopped
      && typeof owner.workerPid === "number" && typeof owner.workerIdentity === "string"
      && await sameProcess(owner.workerPid, owner.workerIdentity);
    if (owner?.sessionId !== hook.session_id && hook.hook_event_name !== "SessionStart" && existingWorkerHealthy) return;
    const workerHealthy = owner?.sessionId === hook.session_id && existingWorkerHealthy;
    const codexChanged = owner?.codexPid !== codex.pid || owner?.codexIdentity !== codex.identity;
    if (!workerHealthy || codexChanged) {
      const replayAfter = owner?.sessionId === hook.session_id && owner.codexIdentity === codex.identity
        ? owner.replayAfter
        : processStartedAt(codex.identity) - 1_000;
      owner = {
        token: randomUUID(),
        sessionId: hook.session_id,
        transcript,
        codexPid: codex.pid,
        codexIdentity: codex.identity,
        replayAfter,
      };
      await mkdir(join(env.directory, owner.sessionId), { recursive: true, mode: 0o700 });
      await atomicJson(ownerPath, owner);
      const log = await open(join(env.directory, "watcher.log"), "a", 0o600);
      try {
        const child = spawn(process.execPath, [SCRIPT, "watch", owner.token], {
          detached: true,
          stdio: ["ignore", "ignore", log.fd],
          env: process.env,
        });
        await new Promise<void>((accept, reject) => {
          child.once("spawn", accept);
          child.once("error", reject);
        });
        owner.workerPid = child.pid;
        owner.workerIdentity = await processIdentity(child.pid!);
        await atomicJson(ownerPath, owner);
        child.unref();
      } finally { await log.close(); }
    }
    if (!owner || owner.sessionId !== hook.session_id || owner.stopped) return;
    if (["SubagentStart", "SubagentStop"].includes(hook.hook_event_name) && typeof hook.agent_id === "string" && ID.test(hook.agent_id)) {
      const registrationPath = join(env.directory, owner.sessionId, `${hook.agent_id}.json`);
      if (!await json(registrationPath)) {
        await atomicJson(registrationPath, {
          id: hook.agent_id,
          name: typeof hook.agent_type === "string" ? hook.agent_type : "subagent",
        } satisfies Registration);
      }
    }
  });
}

export async function watch(token: string): Promise<void> {
  const env = await environment();
  if (!env) return;
  const ownerPath = join(env.directory, "owner.json");
  const initial: Owner | undefined = await json(ownerPath);
  if (!initial || initial.token !== token) return;
  const parent = new JsonlTail();
  const children = new Map<string, { state: ChildState; tail: JsonlTail; path?: string; nextLookup: number; verified: boolean; reconstructed: boolean }>();
  const childErrors = new Map<string, string>();
  let published = "";
  let lastPublished = 0;
  let lastError = "";
  let unavailableSince = 0;
  let nextProbe = 0;
  let ownerProcessHealthy = true;
  let reportUnavailable = false;
  let model: string | null = null;
  const register = (entry: Registration, updateName = false) => {
    if (!ID.test(entry.id)) return;
    if (!children.has(entry.id)) children.set(entry.id, {
      state: { id: entry.id, name: entry.name, activity: "", observed: false },
      tail: new JsonlTail(), nextLookup: 0, verified: false, reconstructed: false,
    });
    else if (updateName && entry.name !== "subagent") children.get(entry.id)!.state.name = entry.name;
  };
  const report = async (states: ChildState[]) => {
    const tokens = { codex_model: model, ...sidebarTokens(states) };
    const serialized = JSON.stringify(tokens);
    if (serialized === published && (Object.values(tokens).every((value) => value === null) || Date.now() - lastPublished < REFRESH_MS)) return;
    await withOwnerLock(env.directory, async () => {
      if ((await json(ownerPath))?.token !== token) return;
      const entries = Object.entries(tokens);
      // Herdr accepts 16 keys per request; keep each child's three keys together.
      for (let offset = 0; offset < entries.length; offset += 16) {
        const args = ["pane", "report-metadata", env.pane, "--source", SOURCE, "--agent", "codex", "--ttl-ms", String(TTL_MS)];
        for (const [key, value] of entries.slice(offset, offset + 16)) args.push(...(value === null ? ["--clear-token", key] : ["--token", `${key}=${value}`]));
        try { await exec(env.binary, args, { timeout: 2_000 }); }
        catch { throw new Error("Herdr metadata update failed"); }
      }
      published = serialized;
      lastPublished = Date.now();
    });
  };
  const verifyParent = (value: unknown): boolean | undefined => {
    const record = object(value);
    if (record?.type !== "session_meta") return;
    const payload = object(record.payload);
    const nested = object(object(object(payload?.source)?.subagent)?.thread_spawn);
    return payload?.parent_thread_id === initial.sessionId && nested?.parent_thread_id === initial.sessionId;
  };
  const staleLifecycle = (value: unknown): boolean => {
    const record = object(value);
    const payload = object(record?.payload);
    if (record?.type !== "event_msg" || !["task_started", "task_complete", "task_failed", "turn_aborted", "shutdown_complete"].includes(payload?.type)) return false;
    const startedAt = typeof payload?.started_at === "number" ? payload.started_at * 1_000 : Date.parse(record.timestamp);
    return Number.isFinite(startedAt) && startedAt < initial.replayAfter;
  };
  try {
    while (true) {
      const owner: Owner | undefined = await json(ownerPath);
      if (!owner || owner.token !== token) return;
      if (owner.stopped || !ownerProcessHealthy) break;
      try {
        if (Date.now() >= nextProbe) {
          ownerProcessHealthy = await sameProcess(owner.codexPid, owner.codexIdentity);
          if (!ownerProcessHealthy) break;
          try { await exec(env.binary, ["pane", "get", env.pane], { timeout: 2_000 }); }
          catch { throw new Error("Herdr parent pane is unavailable"); }
          nextProbe = Date.now() + PROBE_MS;
        }
        for (const value of await parent.read(initial.transcript)) {
          const record = object(value);
          if (record?.type === "turn_context") model = displayText(object(record.payload)?.model) || null;
          const entry = discoveredChild(value);
          if (entry) register(entry, true);
        }
        if (!parent.available) throw new Error("Parent rollout is unavailable");
        const registrationDirectory = join(env.directory, initial.sessionId);
        for (const file of await readdir(registrationDirectory)) {
          if (!file.endsWith(".json")) continue;
          const entry = await json(join(registrationDirectory, file));
          if (typeof entry?.id === "string" && typeof entry.name === "string") register(entry);
        }
        for (const child of children.values()) {
          try {
            if (!child.path && Date.now() >= child.nextLookup) {
              child.path = await findRollout(env.sessions, child.state.id, dirname(initial.transcript));
              child.nextLookup = Date.now() + 5_000;
            }
            child.state.observed = false;
            if (!child.path) continue;
            const records = await child.tail.read(child.path);
            if (child.tail.reset || !child.tail.available) {
              child.verified = false;
              child.reconstructed = false;
            }
            for (const value of records) {
              const verified = verifyParent(value);
              if (verified === false) {
                child.path = undefined;
                child.verified = false;
                child.reconstructed = false;
                child.nextLookup = Date.now() + 5_000;
                break;
              }
              if (verified === true) child.verified = true;
              if (child.verified && !staleLifecycle(value) && consumeChild(child.state, value)) child.reconstructed = true;
            }
            if (child.tail.hasGaps) child.reconstructed = false;
            child.state.observed = child.tail.available && child.tail.caughtUp && child.verified && child.reconstructed;
            childErrors.delete(child.state.id);
          } catch (error) {
            child.state.observed = false;
            const message = error instanceof Error ? error.message : String(error);
            if (childErrors.get(child.state.id) !== message) console.error(`Codex Herdr sidebar (${child.state.id}): ${message}`);
            childErrors.set(child.state.id, message);
          }
        }
        await report([...children.values()].map((child) => child.state));
        reportUnavailable = false;
        lastError = "";
        unavailableSince = 0;
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        if (message !== lastError) console.error(`Codex Herdr sidebar: ${message}`);
        lastError = message;
        const unknown = [...children.values()].map((child) => {
          child.state.observed = false;
          return child.state;
        });
        try { await report(unknown); reportUnavailable = false; }
        catch {
          if (!reportUnavailable) console.error("Codex Herdr sidebar: cannot report unknown state; existing metadata will expire");
          reportUnavailable = true;
        }
        if (reportUnavailable) {
          unavailableSince ||= Date.now();
          if (Date.now() - unavailableSince >= TTL_MS) break;
        } else {
          unavailableSince = 0;
        }
      }
      await Bun.sleep(500);
    }
  } finally {
    model = null;
    await report([]);
  }
}

if (import.meta.main) {
  try {
    if (process.argv[2] === "watch") await watch(process.argv[3]);
    else {
      await handleHook(JSON.parse(await Bun.stdin.text()));
      process.stdout.write("{}\n");
    }
  } catch (error) {
    console.error(`Codex Herdr sidebar: ${error instanceof Error ? error.message : String(error)}`);
    if (process.argv[2] !== "watch") process.stdout.write("{}\n");
  }
}
