import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { settings } from "./module-config";

const packageDir = process.env.PI_INTERACTIVE_SHELL_PACKAGE_DIR;
test.skipIf(!packageDir)("0.18.0 Linux PTY dispatch, notifications, UI, lifecycle and native bash/artifact boundary", () => {
  const source = resolve(packageDir!);
  const manifest = JSON.parse(readFileSync(join(source, "package.json"), "utf8"));
  const pin = settings.packages.find((pkg: { source?: string }) => pkg.source?.startsWith("npm:pi-interactive-shell@"));
  expect(`npm:${manifest.name}@${manifest.version}`).toBe(pin.source);
  expect(pin.skills).toEqual([]);
  expect(pin.prompts).toEqual([]);
  const root = mkdtempSync(join(tmpdir(), "pi-shell-integration-"));
  try {
    for (const dir of ["home", "agent/extensions", "project", "tmp", "cache"]) mkdirSync(join(root, dir), { recursive: true });
    const moduleRoot = fileURLToPath(new URL("../", import.meta.url));
    cpSync(join(moduleRoot, "files/interactive-shell.json"), join(root, "agent/interactive-shell.json"));
    symlinkSync(source, join(root, "agent/extensions/interactive-shell"));
    for (const name of ["rounded-tools.ts", "session-artifacts"]) {
      symlinkSync(join(moduleRoot, "files/extensions", name), join(root, "agent/extensions", name));
    }
    const result = spawnSync("node", [fileURLToPath(new URL("./interactive-shell-fixture.mjs", import.meta.url)),
      fileURLToPath(import.meta.resolve("@earendil-works/pi-coding-agent")), root], {
      cwd: join(root, "project"), encoding: "utf8", timeout: 100_000,
      // Deliberately do not inherit credentials, provider endpoints, or runtime state.
      env: { PATH: process.env.PATH, HOME: join(root, "home"), TMPDIR: join(root, "tmp"),
        XDG_CACHE_HOME: join(root, "cache"), PI_CODING_AGENT_DIR: join(root, "agent"),
        JITI_FS_CACHE: "0", TERM: "xterm-256color" },
    });
    expect(result.error).toBeUndefined();
    expect(result.stderr).toBe("");
    expect(result.status).toBe(0);
    expect(result.stdout).toContain("PASS Linux PTY: quiet exits 0/7, notifications/logs, input/takeover/attach/stop, reload/new/resume/fork/quit, native bash/artifacts, rounded UI; no fixture processes remain");
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 110_000);
