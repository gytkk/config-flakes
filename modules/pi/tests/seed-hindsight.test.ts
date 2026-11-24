import { expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, statSync, symlinkSync, unlinkSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

test("activation seeds a private writable file and preserves local edits and dangling symlinks", () => {
  const home = mkdtempSync(join(tmpdir(), "pi-hindsight-home-"));
  const file = join(home, ".hindsight/coding-agent.json");
  const defaults = fileURLToPath(new URL("../files/hindsight-sepia.example.json", import.meta.url));
  const seed = () => execFileSync("bash", [fileURLToPath(new URL("../files/seed-hindsight.sh", import.meta.url)), defaults], { env: { ...process.env, HOME: home } });
  try {
    seed();
    expect(statSync(file).mode & 0o777).toBe(0o600);
    expect(readFileSync(file, "utf8")).toBe(readFileSync(defaults, "utf8"));
    writeFileSync(file, '{"disabled":true}');
    seed();
    expect(readFileSync(file, "utf8")).toBe('{"disabled":true}');
    unlinkSync(file);
    symlinkSync(join(home, "missing"), file);
    seed();
    expect(existsSync(join(home, "missing"))).toBe(false);
  } finally {
    rmSync(home, { recursive: true, force: true });
  }
});
