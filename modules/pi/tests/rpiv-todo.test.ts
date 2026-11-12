import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { settings } from "./module-config";

const source = process.env.PI_RPIV_TODO_PACKAGE_DIR;
test.skipIf(!source)("pinned rpiv-todo state, replay and actual widget through Node/jiti", () => {
  const packageDir = resolve(source!);
  const manifest = JSON.parse(readFileSync(join(packageDir, "package.json"), "utf8"));
  expect(settings.packages).toContainEqual({ source: `npm:${manifest.name}@${manifest.version}`, skills: [], prompts: [] });
  expect(manifest.name).toBe("@juicesharp/rpiv-todo");
  const root = mkdtempSync(join(tmpdir(), "pi-rpiv-todo-"));
  try {
    for (const dir of ["home", "config", "cache", "project", "agent/extensions"]) mkdirSync(join(root, dir), { recursive: true });
    symlinkSync(packageDir, join(root, "agent/extensions/rpiv-todo"));
    const result = spawnSync("node", [fileURLToPath(new URL("fixtures/rpiv-todo.mjs", import.meta.url)),
      fileURLToPath(import.meta.resolve("@earendil-works/pi-coding-agent")), join(root, "project")], {
      cwd: join(root, "project"), encoding: "utf8", timeout: 30_000,
      env: { PATH: process.env.PATH, HOME: join(root, "home"), XDG_CONFIG_HOME: join(root, "config"),
        XDG_CACHE_HOME: join(root, "cache"), PI_CODING_AGENT_DIR: join(root, "agent"), PI_SUBAGENT_CHILD: "0", JITI_FS_CACHE: "0" },
    });
    if (result.error || result.status !== 0) throw new Error(`${result.error ?? result.status}\n${result.stderr}\n${result.stdout}`);
    expect(result.stderr).toBe("");
    expect(result.stdout).toContain("PASS todo: single registration, mutations/dependencies, branch and reload replay, session isolation, widget/collapse, legacy preservation");
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 35_000);
