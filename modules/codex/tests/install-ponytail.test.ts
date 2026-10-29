import { chmod, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "bun:test";

const script = fileURLToPath(new URL("../files/install-ponytail.sh", import.meta.url));
const list = "plugin list --json";
const marketplace = "plugin marketplace add DietrichGebert/ponytail";
const install = "plugin add ponytail@ponytail";
const installed = (enabled: boolean) => JSON.stringify({
  installed: [{ pluginId: "ponytail@ponytail", installed: true, enabled }],
});

for (const scenario of [
  { name: "preserves an installed plugin", plugins: installed(true), calls: [list], exit: 0 },
  { name: "preserves a disabled plugin", plugins: installed(false), calls: [list], exit: 0 },
  { name: "installs a missing plugin", plugins: '{"installed":[]}', calls: [list, marketplace, install], exit: 0 },
  { name: "stops if listing fails", plugins: installed(true), fail: list, calls: [list], exit: 17 },
  { name: "stops on invalid JSON", plugins: "invalid", calls: [list], exit: 5 },
  { name: "stops on missing installed list", plugins: "{}", calls: [list], exit: 5 },
  { name: "stops if marketplace registration fails", plugins: '{"installed":[]}', fail: marketplace, calls: [list, marketplace], exit: 17 },
  { name: "reports installation failure", plugins: '{"installed":[]}', fail: install, calls: [list, marketplace, install], exit: 17 },
]) {
  test(scenario.name, async () => {
    const temporary = await mkdtemp(join(tmpdir(), "codex-ponytail-"));
    try {
      const codex = join(temporary, "fake codex");
      const log = join(temporary, "calls");
      await writeFile(codex, `#!/bin/sh
printf '%s\n' "$*" >> "$TEST_CALLS"
if [ "$*" = "$TEST_FAIL" ]; then
  echo "Codex command failed: $*" >&2
  exit 17
fi
if [ "$*" = "plugin list --json" ]; then
  printf '%s\n' "$TEST_PLUGINS"
fi
`);
      await chmod(codex, 0o700);
      const child = Bun.spawn(["bash", script, codex, "jq"], {
        env: { ...process.env, TEST_CALLS: log, TEST_PLUGINS: scenario.plugins, TEST_FAIL: scenario.fail ?? "" },
        stdout: "pipe",
        stderr: "pipe",
      });
      const stderr = await new Response(child.stderr).text();
      expect(await child.exited).toBe(scenario.exit);
      expect((await readFile(log, "utf8")).trim().split("\n")).toEqual(scenario.calls);
      if (scenario.exit !== 0) expect(stderr.length).toBeGreaterThan(0);
    } finally {
      await rm(temporary, { recursive: true, force: true });
    }
  });
}
