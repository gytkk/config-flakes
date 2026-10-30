import { createHash, randomUUID } from "node:crypto";
import { chmod, mkdir, mkdtemp, readFile, rename, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "bun:test";

async function atomicJson(path: string, value: unknown): Promise<void> {
  await writeFile(`${path}.tmp`, JSON.stringify(value));
  await rename(`${path}.tmp`, path);
}

async function waitForReports(path: string, expected: string[]): Promise<string> {
  const deadline = Date.now() + 4_000;
  let contents = "";
  while (Date.now() < deadline) {
    contents = await readFile(path, "utf8");
    if (expected.every((value) => contents.includes(value))) return contents;
    await Bun.sleep(25);
  }
  throw new Error(`Timed out waiting for isolated pane reports; log: ${contents}`);
}

test("concurrent panes isolate children in a shared sessions directory", async () => {
  const temporary = await mkdtemp(join(tmpdir(), "codex-herdr-isolation-"));
  const runtime = join(temporary, "runtime");
  const codexHome = join(temporary, "codex");
  const sessions = join(codexHome, "sessions", "2026", "10", "13");
  const socket = join(temporary, "herdr.sock");
  const log = join(temporary, "herdr.log");
  const fixtures = [
    { pane: "context-store-pane", parent: "context-parent", child: "inventory-child", name: "ingest_inventory" },
    { pane: "nix-flakes-pane", parent: "nix-parent", child: "nix-child", name: "inspect_nix" },
  ];
  const watchers: ReturnType<typeof Bun.spawn>[] = [];
  const owners: { path: string; value: Record<string, unknown> }[] = [];
  const jsonl = (records: unknown[]) => records.map((record) => JSON.stringify(record)).join("\n") + "\n";

  try {
    await mkdir(runtime, { recursive: true });
    await mkdir(sessions, { recursive: true });
    await writeFile(log, "");
    await writeFile(join(temporary, "herdr"), "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$HERDR_TEST_LOG\"\n");
    await writeFile(join(temporary, "ps"), "#!/bin/sh\nprintf '%s\\n' 'synthetic-process-identity'\n");
    await chmod(join(temporary, "herdr"), 0o700);
    await chmod(join(temporary, "ps"), 0o700);

    for (const fixture of fixtures) {
      await writeFile(join(sessions, `rollout-${fixture.child}.jsonl`), jsonl([
        {
          type: "session_meta",
          payload: {
            id: fixture.child,
            cwd: temporary,
            parent_thread_id: fixture.parent,
            source: { subagent: { thread_spawn: { parent_thread_id: fixture.parent, agent_path: `/root/${fixture.name}` } } },
          },
        },
        { type: "turn_context", payload: { model: "synthetic-model" } },
        { type: "event_msg", payload: { type: "task_started", turn_id: `${fixture.child}-turn` } },
      ]));
    }

    for (const fixture of fixtures) {
      const transcript = join(sessions, `rollout-${fixture.parent}.jsonl`);
      await writeFile(transcript, jsonl([
        { type: "session_meta", payload: { id: fixture.parent, cwd: temporary } },
        {
          type: "event_msg",
          payload: { type: "item_completed", item: { type: "SubAgentActivity", agent_thread_id: fixture.child, agent_path: `/root/${fixture.name}` } },
        },
      ]));
      const key = createHash("sha256").update(`${socket}\0${fixture.pane}`).digest("hex").slice(0, 24);
      const directory = join(runtime, `codex-herdr-subagents-${process.getuid?.() ?? "user"}`, key);
      await mkdir(join(directory, fixture.parent), { recursive: true });
      const foreign = fixtures.find((other) => other !== fixture)!;
      await atomicJson(join(directory, fixture.parent, `${foreign.child}.json`), { id: foreign.child, name: foreign.name });
      const owner = {
        path: join(directory, "owner.json"),
        value: { token: randomUUID(), sessionId: fixture.parent, transcript, codexPid: process.pid, codexIdentity: "synthetic-process-identity", replayAfter: 0 },
      };
      owners.push(owner);
      await atomicJson(owner.path, owner.value);
      watchers.push(Bun.spawn([
        process.execPath,
        fileURLToPath(new URL("../files/herdr-subagents/index.ts", import.meta.url)),
        "watch",
        owner.value.token,
      ], {
        env: {
          ...process.env,
          HERDR_ENV: "1",
          HERDR_PANE_ID: fixture.pane,
          HERDR_SOCKET_PATH: socket,
          HERDR_BIN_PATH: join(temporary, "herdr"),
          HERDR_TEST_LOG: log,
          CODEX_HOME: codexHome,
          XDG_RUNTIME_DIR: runtime,
          PATH: `${temporary}:${process.env.PATH ?? ""}`,
        },
        stdin: "ignore",
        stdout: "ignore",
        stderr: "inherit",
      }));
    }

    await waitForReports(log, fixtures.map((fixture) => `--token subagent_1=${fixture.name} / synthetic-model`));
    for (const owner of owners) await atomicJson(owner.path, { ...owner.value, stopped: true });
    await waitForReports(log, fixtures.map((fixture) => `pane report-metadata ${fixture.pane} --source codex:herdr-subagents --agent codex --ttl-ms 15000 --clear-token codex_model --clear-token subagent_1_status`));
    const reports = (await readFile(log, "utf8")).split("\n").filter((line) => line.startsWith("pane report-metadata "));
    for (const fixture of fixtures) {
      const paneReports = reports.filter((line) => line.startsWith(`pane report-metadata ${fixture.pane} `));
      expect(paneReports.some((line) => line.includes(`--token subagent_1=${fixture.name} / synthetic-model`))).toBe(true);
      expect(paneReports.some((line) => line.includes("--token subagent_1_status=●"))).toBe(true);
      const foreign = fixtures.find((other) => other !== fixture)!;
      expect(paneReports.some((line) => line.includes(foreign.name))).toBe(false);
    }
  } finally {
    for (const watcher of watchers) watcher.kill();
    await Promise.all(watchers.map((watcher) => watcher.exited));
    await rm(temporary, { recursive: true, force: true });
  }
}, 10_000);
