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

test("installs Korean translation from the checkout without an extra package", () => {
  expect(moduleConfig.enabled.extensionSources[`${prefix}korean-translation`])
    .toBe("/checkout/modules/pi/files/extensions/korean-translation");
});

test("activation settings combine general settings with the explicit package selection", () => {
  expect(baseSettings).not.toHaveProperty("packages");
  const { packages, ...general } = settings;
  expect(general).toEqual(baseSettings);
  expect(packages).toEqual([
    "npm:@juicesharp/rpiv-ask-user-question@2.15.0",
    { source: "npm:@juicesharp/rpiv-todo@2.15.0", skills: [], prompts: [] },
    { source: "npm:pi-lens@4.4.12", skills: [], prompts: [] },
    { source: "npm:pi-interactive-shell@0.18.0", skills: [], prompts: [] },
    "npm:pi-web-access@0.46.0",
    { source: "npm:pi-subagents@0.84.0", skills: [], prompts: [] },
    "npm:pi-claude-subscription@0.2.4",
  ]);
  expect(moduleConfig.enabled.activation).toEqual({ before: ["linkGeneration"], after: ["writeBoundary"] });
});

test("Devsisters selects Opus through Claude SDK without changing package selection", () => {
  const { defaultProvider, defaultModel, packages, subagents } = moduleConfig.devsisters.settings;
  expect([defaultProvider, defaultModel]).toEqual(["claude-sdk", "claude-opus-5-5"]);
  expect(packages).toEqual(settings.packages);
  expect(subagents.defaultModel).toBe("claude-sdk/claude-opus-5-5");
  expect(subagents.modelScope).toEqual({ enforce: true, strict: true, allow: ["claude-sdk/*"] });
  for (const role of Object.values(subagents.agentOverrides) as { model?: string }[]) {
    if (role.model) expect(role.model).toMatch(/^claude-sdk\//);
  }
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

test("local prompt UI keeps hardware cursor settings without Powerline configuration", () => {
  for (const key of ["powerline", "bashMode", "workingVibe"]) expect(settings).not.toHaveProperty(key);
  expect(settings.editorPaddingX).toBe(1);
  expect(settings.outputPad).toBe(1);
  expect(settings.showHardwareCursor).toBe(true);
});

test("local extension names map to their existing checkout source", () => {
  const sources = moduleConfig.enabled.extensionSources as Record<string, string>;
  for (const name of ["auto-title.ts", "codex-fast-mode.ts", "subscription-usage.ts", "rounded-tools.ts", "herdr-subagents.ts", "terminal-title-reset.ts"]) {
    expect(sources[`${prefix}${name}`]).toBeDefined();
  }
  expect(sources[`${prefix}pi-lsp`]).toBeUndefined();
  expect(sources[`${prefix}ast-transform`]).toBeUndefined();
  expect(sources[`${prefix}phase-tasks`]).toBeUndefined();
  expect(sources[`${prefix}herdr-agent-state.ts`]).toBe("/herdr/src/integration/assets/pi/herdr-agent-state.ts");
  for (const [target, source] of Object.entries(sources)) {
    if (target === `${prefix}herdr-agent-state.ts`) continue;
    expect(source).toBe(`/checkout/modules/pi/files/extensions/${target.slice(prefix.length)}`);
  }
  expect(sources[`${prefix}subagent/config.json`]).toBe("/checkout/modules/pi/files/extensions/subagent/config.json");
  expect(sources[`${prefix}lsp-refactor`]).toBeUndefined();
  expect(sources[`${prefix}prompt-ui`]).toBe("/checkout/modules/pi/files/extensions/prompt-ui");
});

test("session recording requires both the recorder and the Pi provider", () => {
  const key = `${prefix}session-record.ts`;
  expect(moduleConfig.enabled.extensionSources[key]).toBeUndefined();
  expect(moduleConfig.piRecordingOff.extensionSources[key]).toBeUndefined();
  expect(moduleConfig.recording.extensionSources[key]).toBe("/checkout/modules/pi/files/extensions/session-record.ts");
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
    writeFileSync(local, JSON.stringify({ deviceId: "test-device", localSetting: true, packages: ["npm:pi-powerline-footer@0.22.0"], powerline: { preset: "default" }, bashMode: { completions: false }, workingVibe: "off" }));
    const result = spawnSync("bash", [fileURLToPath(new URL("../files/sync-settings.sh", import.meta.url)), common, local], { encoding: "utf8" });
    if (result.error) throw result.error;
    expect(result.stderr).toBe("");
    expect(result.status).toBe(0);
    expect(JSON.parse(readFileSync(local, "utf8"))).toEqual({ ...settings, deviceId: "test-device", localSetting: true });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("only a launch-environment profile installs the extension, and a pinned Pi skips the package", () => {
  expect(moduleConfig.enabled.extensionSources[`${prefix}launch-environment.ts`]).toBeUndefined();
  expect(moduleConfig.enabled.packages).toContain("pi");
  expect(moduleConfig.jari.packages).not.toContain("pi");
  expect(moduleConfig.jari.packages).toEqual(moduleConfig.enabled.packages.filter((name: string) => name !== "pi"));
  const { [`${prefix}launch-environment.ts`]: launch, ...otherSources } = moduleConfig.jari.extensionSources;
  expect(launch).toBeDefined();
  expect(otherSources).toEqual(moduleConfig.disabled.extensionSources);
  expect(moduleConfig.jari.settings).toEqual(moduleConfig.devsisters.settings);
});

test("the launch-environment extension sets session variables and moves its PATH entries to the front once", async () => {
  const saved = { ...process.env };
  try {
    for (const name of Object.keys(moduleConfig.jari.sessionVariables)) delete process.env[name];
    process.env.PATH = "/usr/bin:/profile/bin::/bin:/opt/jari/bin";
    const { default: apply } = await import(moduleConfig.jari.extensionSources[`${prefix}launch-environment.ts`]);
    apply();
    apply();
    expect(process.env.PATH).toBe("/opt/jari/bin:/profile/bin:/usr/bin:/bin");
    expect(moduleConfig.jari.sessionVariables).toEqual({ ...moduleConfig.enabled.sessionVariables, PI_UNPATCHED: "1" });
    for (const [name, value] of Object.entries(moduleConfig.jari.sessionVariables)) expect(process.env[name]).toBe(value);
  } finally {
    for (const name of Object.keys(process.env)) if (!(name in saved)) delete process.env[name];
    Object.assign(process.env, saved);
  }
});
