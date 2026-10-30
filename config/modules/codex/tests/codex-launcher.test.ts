import { chmod, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "bun:test";

test("Herdr launches Codex locally and preserves arguments and exit status", async () => {
  const temporary = await mkdtemp(join(tmpdir(), "codex-launcher-"));
  try {
    const binary = join(temporary, "fake codex");
    await writeFile(binary, "#!/bin/sh\nprintf '%s\\0' \"$@\"\nexit 17\n");
    await chmod(binary, 0o700);
    const source = await readFile(new URL("../files/codex-launcher.sh", import.meta.url), "utf8");
    const launcher = join(temporary, "launcher.sh");
    await writeFile(launcher, source.replace("@codex@", `'${binary.replaceAll("'", "'\\''")}'`));
    for (const herdr of ["1", "0", ""]) {
      for (const [args, localArgs] of [
        [["resume", "session-id"], ["--no-daemon", "resume", "session-id"]],
        [["exec", "a prompt with spaces", "--json"], ["--no-daemon", "exec", "a prompt with spaces", "--json"]],
        [["--help"], ["--no-daemon", "--help"]],
        [["--no-daemon", "resume", "session-id"], ["--no-daemon", "resume", "session-id"]],
        [["resume", "session-id", "--no-daemon"], ["resume", "session-id", "--no-daemon"]],
        [["--", "--no-daemon"], ["--no-daemon", "--", "--no-daemon"]],
      ]) {
        const child = Bun.spawn(["sh", launcher, ...args], {
          env: { ...process.env, HERDR_ENV: herdr },
          stdout: "pipe",
          stderr: "inherit",
        });
        const actual = (await new Response(child.stdout).text()).split("\0").slice(0, -1);
        expect(actual).toEqual(herdr === "1" ? localArgs : args);
        expect(await child.exited).toBe(17);
      }
    }
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
});
