import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { settings } from "./module-config";

// Deliberately opt-in: no download and no access to the user's installed packages.
const source = process.env.PI_LENS_PACKAGE_DIR;
test.skipIf(!source)("Lens exact-release loader, policy, tools and real server lifecycle", () => {
  const root = mkdtempSync(join(tmpdir(), "pi-lens-test-"));
  try {
    const pkg = resolve(source!);
    const manifest = JSON.parse(readFileSync(join(pkg, "package.json"), "utf8"));
    const pin = settings.packages.find((entry: any) => entry.source?.startsWith("npm:pi-lens@"));
    expect(pin.source).toBe(`npm:${manifest.name}@${manifest.version}`);
    for (const dir of ["home/.pi-lens", "agent/npm", "agent/extensions", "cache", "data", "project", "lens"]) mkdirSync(join(root, dir), { recursive: true });
    symlinkSync(dirname(pkg), join(root, "agent/npm/node_modules"));
    symlinkSync(fileURLToPath(new URL("../files/extensions/rounded-tools.ts", import.meta.url)), join(root, "agent/extensions/rounded-tools.ts"));
    writeFileSync(join(root, "agent/settings.json"), JSON.stringify({ packages: [pin], defaultProjectTrust: "yes" }));
    const config = fileURLToPath(new URL("../files/lens.json", import.meta.url));
    writeFileSync(join(root, "home/.pi-lens/config.json"), readFileSync(config));
    const env = {
      // Allowlist executable discovery only: no inherited API keys, auth/provider
      // overrides, proxy settings, NODE_OPTIONS, or user package-manager config.
      PATH: process.env.PATH!, LANG: "C.UTF-8",
      HOME: join(root, "home"), USERPROFILE: join(root, "home"),
      XDG_CONFIG_HOME: join(root, "home/.config"), XDG_CACHE_HOME: join(root, "cache"), XDG_STATE_HOME: join(root, "home/.local/state"),
      PI_CODING_AGENT_DIR: join(root, "agent"), PI_LENS_HOME: join(root, "lens"), PILENS_DATA_DIR: join(root, "data"),
      PI_LENS_CONFIG_PATH: join(root, "home/.pi-lens/config.json"), PI_LENS_INSTALL_LOG: join(root, "lens/install.log"),
      npm_config_cache: join(root, "cache/npm"), npm_config_offline: "true", JITI_FS_CACHE: "0", PI_SUBAGENT_CHILD: "0",
      PI_LENS_DISABLE_LSP_INSTALL: "1", PI_LENS_DISABLE_TOOL_INSTALL: "1", PI_OFFLINE: "1",
      GOCACHE: join(root, "cache/go-build"), GOPATH: join(root, "cache/go"), GOPROXY: "off", GOTOOLCHAIN: "local", CARGO_HOME: join(root, "cache/cargo"),
      PI_LENS_LSP_NAV_REQUEST_TIMEOUT_MS: "3000",
      CARGO_TARGET_DIR: join(root, "cache/target"), CARGO_NET_OFFLINE: "true",
      // Use the existing toolchain read-only; rustup must never install one.
      RUSTUP_HOME: process.env.RUSTUP_HOME || join(process.env.HOME!, ".rustup"),
    };
    const result = spawnSync("node", [fileURLToPath(new URL("fixtures/lens.mjs", import.meta.url)),
      fileURLToPath(import.meta.resolve("@earendil-works/pi-coding-agent")), root], {
      env, cwd: join(root, "project"), encoding: "utf8", timeout: 240_000, maxBuffer: 8 * 1024 * 1024,
    });
    console.log(result.stdout);
    if (result.error || result.status !== 0) throw new Error(`${result.error ?? result.status}\n${result.stderr}\n${result.stdout}`);
    expect(result.stdout).toContain("LENS CHECKS PASSED");
  } finally { rmSync(root, { recursive: true, force: true }); }
}, 250_000);
