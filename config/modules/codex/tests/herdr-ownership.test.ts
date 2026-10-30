import { createHash } from "node:crypto";
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "bun:test";

test.each([
  ["codex app-server --listen unix:// --managed-daemon", true],
  ["codex --no-daemon Explain --managed-daemon behavior", false],
])("hook ownership distinguishes managed daemons from local prompts: %s", async (args, daemon) => {
  const temporary = await mkdtemp(join(tmpdir(), "codex-herdr-ownership-"));
  const socket = join(temporary, "herdr.sock");
  const pane = "nix-flakes-pane";
  const key = createHash("sha256").update(`${socket}\0${pane}`).digest("hex").slice(0, 24);
  const directory = join(temporary, `codex-herdr-subagents-${process.getuid?.() ?? "user"}`, key);
  const ownerPath = join(directory, "owner.json");
  const transcript = join(temporary, "context-store.jsonl");
  const original = {
    token: "nix-flakes-owner",
    sessionId: "nix-flakes-session",
    transcript: join(temporary, "nix-flakes.jsonl"),
    codexPid: process.pid,
    codexIdentity: "synthetic-process-identity",
    replayAfter: 0,
  };
  try {
    await mkdir(directory, { recursive: true });
    await writeFile(ownerPath, JSON.stringify(original));
    await writeFile(transcript, `${JSON.stringify({
      type: "session_meta", payload: { id: "context-store-session", cwd: "/context-store" },
    })}\n`);
    const ps = join(temporary, "ps");
    await writeFile(ps, [
      "#!/bin/sh",
      "case \"$*\" in",
      "  *args=*) printf '%s\\n' \"$CODEX_TEST_ARGS\" ;;",
      "  *ppid=*) printf '%s\\n' '1 /codex/app-server-daemon/bin/codex' ;;",
      "  *) printf '%s\\n' 'synthetic-process-identity' ;;",
      "esac",
      "",
    ].join("\n"));
    await chmod(ps, 0o700);
    const hook = Bun.spawn([
      process.execPath,
      fileURLToPath(new URL("../files/herdr-subagents/index.ts", import.meta.url)),
    ], {
      env: {
        ...process.env,
        CODEX_THREAD_ID: "",
        CODEX_TEST_ARGS: args,
        HERDR_ENV: "1",
        HERDR_PANE_ID: pane,
        HERDR_SOCKET_PATH: socket,
        HERDR_BIN_PATH: "/nonexistent-test-herdr",
        XDG_RUNTIME_DIR: temporary,
        PATH: `${temporary}:${process.env.PATH ?? ""}`,
      },
      stdin: new Response(JSON.stringify({
        hook_event_name: "SessionStart",
        session_id: "context-store-session",
        transcript_path: transcript,
      })),
      stdout: "pipe",
      stderr: "pipe",
    });
    const stderr = await new Response(hook.stderr).text();
    expect(await hook.exited).toBe(0);
    const owner = JSON.parse(await readFile(ownerPath, "utf8"));
    if (daemon) {
      expect(owner).toEqual(original);
      expect(stderr).toContain("--no-daemon");
    } else {
      expect(owner.sessionId).toBe("context-store-session");
      expect(owner.workerPid).toBeNumber();
      expect(stderr).toBe("");
    }
  } finally {
    // Stop only a watcher created by this fixture.
    const owner = JSON.parse(await readFile(ownerPath, "utf8"));
    if (owner.token !== original.token && owner.workerPid) {
      try { process.kill(owner.workerPid, "SIGTERM"); }
      catch (error: any) { if (error.code !== "ESRCH") throw error; }
    }
    await rm(temporary, { recursive: true, force: true });
  }
}, 10_000);
