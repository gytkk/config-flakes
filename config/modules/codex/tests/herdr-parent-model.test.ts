import { createHash, randomUUID } from "node:crypto";
import {
  appendFile,
  chmod,
  mkdir,
  mkdtemp,
  readFile,
  rename,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "bun:test";

async function waitForLog(path: string, text: string, timeoutMs = 4_000): Promise<string> {
  const deadline = Date.now() + timeoutMs;
  let contents = "";
  while (Date.now() < deadline) {
    contents = await readFile(path, "utf8").catch(() => "");
    if (contents.includes(text)) return contents;
    await Bun.sleep(25);
  }
  throw new Error(`Timed out waiting for fake Herdr log to contain: ${text}; log: ${contents}`);
}

async function atomicJson(path: string, value: unknown): Promise<void> {
  const temporary = `${path}.tmp`;
  await writeFile(temporary, JSON.stringify(value));
  await rename(temporary, path);
}

async function withTimeout<T>(promise: Promise<T>, timeoutMs: number, message: string): Promise<T> {
  let timeout: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      promise,
      new Promise<never>((_, reject) => {
        timeout = setTimeout(() => reject(new Error(message)), timeoutMs);
      }),
    ]);
  } finally {
    if (timeout) clearTimeout(timeout);
  }
}

test("watch reports parent model changes and clears the model when it stops", async () => {
  const temporary = await mkdtemp(join(tmpdir(), "codex-herdr-parent-model-"));
  const runtime = join(temporary, "runtime");
  const transcript = join(temporary, "parent.jsonl");
  const log = join(temporary, "herdr.log");
  const fakeHerdr = join(temporary, "herdr");
  const fakePs = join(temporary, "ps");
  const socket = join(temporary, "herdr.sock");
  const pane = "pane-parent-model-test";
  const sessionId = "session-parent-model-test";
  const token = randomUUID();
  let watcher: ReturnType<typeof Bun.spawn> | undefined;
  let ownerPath: string | undefined;
  let owner: Record<string, unknown> | undefined;

  try {
    await mkdir(runtime, { recursive: true });
    await writeFile(log, "");
    await writeFile(fakeHerdr, [
      "#!/bin/sh",
      "printf '%s\\n' \"$*\" >> \"$HERDR_TEST_LOG\"",
      "exit 0",
      "",
    ].join("\n"));
    await chmod(fakeHerdr, 0o700);
    await writeFile(fakePs, [
      "#!/bin/sh",
      "printf '%s\\n' 'synthetic-process-identity'",
      "",
    ].join("\n"));
    await chmod(fakePs, 0o700);
    await writeFile(transcript, `${JSON.stringify({
      type: "turn_context",
      payload: { model: "synthetic-model-one" },
    })}\n`);

    const key = createHash("sha256").update(`${socket}\0${pane}`).digest("hex").slice(0, 24);
    const watcherDirectory = join(runtime, `codex-herdr-subagents-${process.getuid?.() ?? "user"}`, key);
    ownerPath = join(watcherDirectory, "owner.json");
    owner = {
      token,
      sessionId,
      transcript,
      codexPid: process.pid,
      codexIdentity: "synthetic-process-identity",
      replayAfter: 0,
    };
    await mkdir(join(watcherDirectory, sessionId), { recursive: true });
    await atomicJson(ownerPath, owner);

    watcher = Bun.spawn([
      process.execPath,
      fileURLToPath(new URL("../files/herdr-subagents/index.ts", import.meta.url)),
      "watch",
      token,
    ], {
      env: {
        ...process.env,
        HERDR_ENV: "1",
        HERDR_SOCKET_PATH: socket,
        HERDR_PANE_ID: pane,
        HERDR_BIN_PATH: fakeHerdr,
        HERDR_TEST_LOG: log,
        XDG_RUNTIME_DIR: runtime,
        PATH: `${temporary}:${process.env.PATH ?? ""}`,
      },
      stdin: "ignore",
      stdout: "ignore",
      stderr: "inherit",
    });
    await waitForLog(log, "--token codex_model=synthetic-model-one");

    await appendFile(transcript, `${JSON.stringify({
      type: "turn_context",
      payload: { model: "synthetic-model-two" },
    })}\n`);
    await waitForLog(log, "--token codex_model=synthetic-model-two");

    await atomicJson(ownerPath, { ...owner, stopped: true });
    expect(await withTimeout(watcher.exited, 4_000, "Watcher did not stop after its owner stopped")).toBe(0);
    const reports = await waitForLog(log, "--clear-token codex_model");
    expect(reports).toContain(`pane report-metadata ${pane} --source codex:herdr-subagents --agent codex`);
    expect(reports).toContain("--token codex_model=synthetic-model-one");
    expect(reports).toContain("--token codex_model=synthetic-model-two");
  } finally {
    if (ownerPath && owner) await atomicJson(ownerPath, { ...owner, stopped: true }).catch(() => {});
    if (watcher) {
      try {
        await withTimeout(watcher.exited, 4_000, "Watcher did not stop during test cleanup");
      } catch {
        watcher.kill();
        await watcher.exited;
      }
    }
    await rm(temporary, { recursive: true, force: true });
  }
}, 10_000);
