import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import baseSettings from "../files/settings.json";
import shellConfig from "../files/interactive-shell.json";
import { moduleConfig, settings } from "./module-config";

const prefix = ".pi/agent/extensions/";

test("activation settings combine general settings with the explicit package selection", () => {
  expect(baseSettings).not.toHaveProperty("packages");
  const { packages, ...general } = settings;
  expect(general).toEqual(baseSettings);
  expect(packages).toEqual([
    "npm:@juicesharp/rpiv-ask-user-question@2.15.0",
    { source: "npm:pi-lens@4.4.12", skills: [], prompts: [] },
    { source: "npm:pi-interactive-shell@0.18.0", skills: [], prompts: [] },
    "npm:pi-powerline-footer@0.22.0",
    "npm:pi-web-access@0.46.0",
    { source: "npm:pi-subagents@0.84.0", skills: [], prompts: [] },
  ]);
  expect(moduleConfig.enabled.activation).toEqual({ before: ["linkGeneration"], after: ["writeBoundary"] });
});

test("interactive shell uses managed lean, deferred, local-only configuration", () => {
  expect(moduleConfig.enabled.interactiveShellSource).toBe("/checkout/modules/pi/files/interactive-shell.json");
  expect(moduleConfig.disabled.interactiveShellSource).toBe(moduleConfig.enabled.interactiveShellSource);
  expect(shellConfig).toEqual({ defer: true, toolSchema: "lean", jev: { enabled: false } });
});

test("Lens owns the canonical config and runtime installs are disabled", () => {
  expect(moduleConfig.enabled.sessionVariables).toEqual({ PI_LENS_DISABLE_LSP_INSTALL: "1", PI_LENS_DISABLE_TOOL_INSTALL: "1" });
  expect(moduleConfig.enabled.lensConfigSource).toBe("/checkout/modules/pi/files/lens.json");
  expect(moduleConfig.enabled.legacyLspConfig).toBe(false);
});

test("Powerline defaults keep optional execution features off and status providers separate", () => {
  expect(settings.powerline).toEqual({
    preset: "default", placement: "above", welcome: false,
    separator: "pipe", path: { mode: "full" },
    layout: {
      left: ["path", "git", "context_pct"],
      right: ["extension_statuses", "custom:weekly", "model", "thinking", "custom:fast"],
      secondary: [],
    },
    queue: { compactPromptMode: "native" }, sendDelayMs: 0, autoFollowUp: false,
    customItems: [
      { id: "weekly", statusKey: "codex-usage", position: "right" },
      { id: "fast", statusKey: "codex-fast-mode", position: "right", color: "warning" },
    ],
  });
  expect(settings.bashMode).toEqual({ toggleShortcut: null, completions: false });
  expect(settings.workingVibe).toBe("off");
  expect(settings.editorPaddingX).toBe(1);
  expect(settings.outputPad).toBe(1);
  expect(settings.showHardwareCursor).toBe(true);
});

test("local extension names map to their existing checkout source", () => {
  const sources = moduleConfig.enabled.extensionSources as Record<string, string>;
  for (const name of ["codex-fast-mode.ts", "codex-usage.ts", "rounded-tools.ts", "herdr-subagents.ts", "terminal-title-reset.ts"]) {
    expect(sources[`${prefix}${name}`]).toBeDefined();
  }
  expect(sources[`${prefix}pi-lsp`]).toBeUndefined();
  expect(sources[`${prefix}ast-transform`]).toBeUndefined();
  expect(sources[`${prefix}herdr-agent-state.ts`]).toBe("/herdr/src/integration/assets/pi/herdr-agent-state.ts");
  for (const [target, source] of Object.entries(sources)) {
    if (target === `${prefix}herdr-agent-state.ts`) continue;
    expect(source).toBe(`/checkout/modules/pi/files/extensions/${target.slice(prefix.length)}`);
  }
  expect(sources[`${prefix}subagent/config.json`]).toBe("/checkout/modules/pi/files/extensions/subagent/config.json");
  expect(sources[`${prefix}lsp-refactor`]).toBeUndefined();
  expect(sources[`${prefix}prompt-ui`]).toBeUndefined();
});

test("disabling Herdr removes only its official state extension", () => {
  const { [`${prefix}herdr-agent-state.ts`]: state, ...otherSources } = moduleConfig.enabled.extensionSources;
  expect(state).toBeDefined();
  expect(moduleConfig.disabled.extensionSources).toEqual(otherSources);
  expect(moduleConfig.disabled.settings).toEqual(settings);
});

test("generated settings replace managed packages while preserving device-local metadata", () => {
  const root = mkdtempSync(join(tmpdir(), "pi-module-settings-"));
  try {
    const common = join(root, "common.json");
    const local = join(root, "settings.json");
    writeFileSync(common, JSON.stringify(settings));
    writeFileSync(local, JSON.stringify({ deviceId: "test-device", localSetting: true, packages: ["npm:local-only"] }));
    const result = spawnSync("bash", [fileURLToPath(new URL("../files/sync-settings.sh", import.meta.url)), common, local], { encoding: "utf8" });
    if (result.error) throw result.error;
    expect(result.stderr).toBe("");
    expect(result.status).toBe(0);
    expect(JSON.parse(readFileSync(local, "utf8"))).toEqual({ ...settings, deviceId: "test-device", localSetting: true });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
