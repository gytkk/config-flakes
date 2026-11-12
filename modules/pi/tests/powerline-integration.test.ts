import { expect, mock, test } from "bun:test";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { CustomEditor } from "@earendil-works/pi-coding-agent";
import { CombinedAutocompleteProvider, CURSOR_MARKER, stripTerminalSequences, visibleWidth } from "@earendil-works/pi-tui";
import fastMode from "../files/extensions/codex-fast-mode";
import codexUsage from "../files/extensions/codex-usage";
import { settings } from "./module-config";

const source = process.env.PI_POWERLINE_PACKAGE_DIR;
// Exercise the shipped components, not a copy of the upstream renderer. No provider/auth calls.
async function withPowerline(run: (fixture: any) => Promise<void>) {
  if (!source) throw new Error("Set PI_POWERLINE_PACKAGE_DIR to the unpacked pi-powerline-footer@0.22.0 release.");
  const manifest = JSON.parse(readFileSync(join(source!, "package.json"), "utf8"));
  expect(`${manifest.name}@${manifest.version}`).toBe("pi-powerline-footer@0.22.0");
  const root = mkdtempSync(join(tmpdir(), "pi-powerline-ui-"));
  const savedEnv = { HOME: process.env.HOME, PI_CODING_AGENT_DIR: process.env.PI_CODING_AGENT_DIR, PI_SUBAGENT_CHILD: process.env.PI_SUBAGENT_CHILD };
  process.env.HOME = root;
  process.env.PI_CODING_AGENT_DIR = join(root, "agent");
  process.env.PI_SUBAGENT_CHILD = "0";
  mkdirSync(process.env.PI_CODING_AGENT_DIR);
  writeFileSync(join(process.env.PI_CODING_AGENT_DIR, "settings.json"), JSON.stringify(settings));
  const cwd = join(root, "fixture-project");
  mkdirSync(cwd);
  const git = (...args: string[]) => execFileSync("git", ["-C", cwd, ...args], { encoding: "utf8" });
  git("init", "-q", "-b", "main");
  writeFileSync(join(cwd, "tracked"), "initial\n");
  git("add", ".");
  git("-c", "user.name=Test", "-c", "user.email=test@example.invalid", "-c", "commit.gpgSign=false", "commit", "-qm", "initial");
  writeFileSync(join(cwd, "tracked"), "staged\n");
  git("add", "tracked");
  writeFileSync(join(cwd, "tracked"), "unstaged\n");
  writeFileSync(join(cwd, "untracked"), "new\n");
  const handlers = new Map<string, Function[]>();
  const commands = new Map<string, any>();
  let editor: any;
  let footer: any;
  let editorFactory: Function | undefined;
  let terminalInput: Function | undefined;
  const widgets = new Map<string, any>();
  const statuses = new Map<string, string>();
  const registrations = { editor: 0, footer: 0 };
  const entries: any[] = [];
  const sent: string[] = [];
  const providerCall = mock(() => { throw new Error("Unexpected model call"); });
  const piRoot = dirname(fileURLToPath(import.meta.resolve("@earendil-works/pi-coding-agent")));
  const { initTheme, theme, getEditorTheme } = await import(join(piRoot, "modes/interactive/theme/theme.js"));
  const { KeybindingsManager } = await import(join(piRoot, "core/keybindings.js"));
  initTheme("light", false);
  const keybindings = new KeybindingsManager();
  const tui = { terminal: { rows: 24, columns: 180 }, requestRender() {}, hasOverlay: () => false };
  const footerData = {
    getGitBranch: () => "main", getExtensionStatuses: () => statuses,
    onBranchChange: () => () => {},
    setExtensionStatus(key: string, value?: string) {
      if (value === undefined) statuses.delete(key); else statuses.set(key, value);
    },
  };
  const ctx: any = {
    mode: "tui", hasUI: true, cwd, thinkingLevel: "high",
    model: { id: "fixture-model", name: "Fixture Model", provider: "openai", reasoning: true, contextWindow: 272000 },
    sessionManager: {
      getCwd: () => cwd, getSessionId: () => "fixture", getSessionName: () => undefined,
      getBranch: () => entries, getEntries: () => entries, getLeafId: () => String(entries.length),
      buildContextEntries: () => [],
    },
    modelRegistry: { isUsingOAuth: () => true, complete: providerCall, streamSimple: providerCall },
    getContextUsage: () => ({ tokens: 68000, contextWindow: ctx.model.contextWindow, percent: 68000 / ctx.model.contextWindow * 100 }),
    getSystemPrompt: () => "fixture", isIdle: () => true,
    compact: mock(() => {}),
    ui: {
      theme, notify: mock(() => {}), custom: mock(() => { throw new Error("Unexpected welcome"); }),
      setWorkingMessage: mock(() => {}), setHeader: mock(() => {}),
      setStatus: (key: string, value?: string) => footerData.setExtensionStatus(key, value),
      onTerminalInput: (handler: Function) => { terminalInput = handler; return () => { terminalInput = undefined; }; },
      getEditorComponent: () => editorFactory,
      getEditorText: () => editor?.getText() ?? "",
      setEditorText: (text: string) => editor?.setText(text),
      setEditorComponent: (factory: Function) => {
        registrations.editor++;
        editorFactory = factory;
        editor = factory(tui, getEditorTheme(), keybindings);
        editor.setPaddingX(settings.editorPaddingX);
        editor.focused = true;
        editor.onSubmit = (text: string) => sent.push(text);
      },
      setFooter: (factory: Function) => { registrations.footer++; footer = factory(tui, theme, footerData); },
      setWidget: (key: string, factory?: Function, options?: any) => {
        if (!factory) widgets.delete(key);
        else widgets.set(key, { component: factory(tui, theme), options });
      },
    },
  };
  let usageResult = { code: 0, killed: false, stderr: "", stdout: JSON.stringify([{
    provider: "codex", usage: { secondary: { usedPercent: 21 }, updatedAt: new Date().toISOString() },
  }]) };
  const exec = mock(async () => usageResult);
  const pi: any = {
    on: (name: string, handler: Function) => handlers.set(name, [...(handlers.get(name) ?? []), handler]),
    registerCommand: (name: string, command: any) => { expect(commands.has(name)).toBe(false); commands.set(name, command); },
    appendEntry: (customType: string, data: any) => entries.push({ type: "custom", customType, data }),
    sendUserMessage: mock(() => { throw new Error("Unexpected automatic send"); }),
    exec,
  };
  const emit = async (name: string, event = {}) => {
    const results = [];
    for (const handler of handlers.get(name) ?? []) results.push(await handler(event, ctx));
    return results;
  };
  const render = (width = 220) => {
    widgets.get("powerline-top").component.invalidate();
    return [...widgets.get("powerline-top").component.render(width), ...footer.render(width)];
  };
  const text = () => render().map(stripTerminalSequences).join("\n");
  const command = (name: string, args = "") => commands.get(name).handler(args, ctx);
  let gitModule: any;
  try {
    const upstream = await import(join(source!, "index.ts"));
    const { parsePowerlineConfig } = await import(join(source!, "powerline-config.ts"));
    const parsed = parsePowerlineConfig(settings.powerline, ["default"]);
    expect(parsed.invalidLayoutSegments).toEqual([]);
    expect(parsed.invalidDisabledSegments).toEqual([]);
    expect(parsed.welcome).toBe(false);
    expect(parsed.queue.compactPromptMode).toBe("native");
    expect(parsed.sendDelayMs).toBe(0);
    expect(parsed.autoFollowUp).toBe(false);
    expect(upstream.parseBashModeSettings(settings).toggleShortcut).toBeNull();
    expect(upstream.parseBashModeSettings(settings).completions).toBe(false);
    fastMode(pi);
    upstream.default(pi);
    codexUsage(pi);
    await emit("session_start", { reason: "startup" });
    expect(registrations).toEqual({ editor: 1, footer: 1 });
    expect(widgets.get("powerline-top").options.placement).toBe("aboveEditor");
    expect(ctx.ui.custom).not.toHaveBeenCalled();
    expect(ctx.ui.notify).not.toHaveBeenCalled();
    render();
    gitModule = await import(join(source!, "git-status.ts"));
    await gitModule.waitForGitUpdates();
    await run({ ctx, emit, command, render, text, exec, pi, providerCall, sent, keybindings,
      get editor() { return editor; }, get terminalInput() { return terminalInput; },
      setUsageResult: (result: typeof usageResult) => { usageResult = result; },
      get usageResult() { return usageResult; },
      reload: async () => {
        await emit("session_shutdown", { reason: "reload" });
        footer.dispose();
        editorFactory = undefined; // Pi tears down the old runtime before reloading.
        await emit("session_start", { reason: "reload" });
      },
      makeBaseEditor: () => new CustomEditor(tui as never, getEditorTheme(), keybindings),
    });
  } finally {
    await emit("session_shutdown", { reason: "quit" });
    footer?.dispose();
    await gitModule?.waitForGitUpdates();
    for (const [key, value] of Object.entries(savedEnv)) {
      if (value === undefined) delete process.env[key]; else process.env[key] = value;
    }
    rmSync(root, { recursive: true, force: true });
  }
}

test("Powerline 0.22.0 renders provider statuses and preserves native input paths", () => withPowerline(async f => {
  const { ctx, emit, command, render, text, exec, pi, providerCall, sent, keybindings, editor, terminalInput } = f;
  const initial = text();
  for (const value of ["Fixture Model", "high", "fixture-project", "main", "+1", "*1", "?1", "272k", "25", "79%"]) expect(initial).toContain(value);
  expect(initial.match(/79%/g)).toHaveLength(1);
  expect(initial).not.toContain("fast");
  await command("fast", "on");
  expect(text().match(/fast/g)).toHaveLength(1);
  ctx.ui.setStatus("layout-fixture", "extension fixture");
  const ordered = text();
  const items = ["~/fixture-project", "main", "272k", "extension fixture", "79%", "Fixture Model", "think:high", "fast"];
  const positions = items.map(item => ordered.indexOf(item));
  expect(positions.every(position => position >= 0)).toBe(true);
  expect(positions.every((position, index) => index === 0 || position > positions[index - 1])).toBe(true);
  expect(ordered).toMatch(/[│|]/);
  expect(ordered).not.toContain("(sub)");
  expect(ordered).not.toContain("cache");
  ctx.ui.setStatus("layout-fixture", undefined);
  ctx.model = { ...ctx.model, name: "Other Model", provider: "anthropic", contextWindow: 200000 };
  await emit("model_select");
  expect(text()).toContain("Other Model");
  expect(text()).toContain("200k");
  expect(text()).not.toContain("Fixture Model");
  expect(text()).not.toContain("fast");
  ctx.model.provider = "openai";
  await emit("model_select");
  expect(text()).toContain("fast");
  await command("fast", "off");
  expect(text()).not.toContain("fast");
  ctx.thinkingLevel = "low";
  await emit("thinking_level_select", { level: "low" });
  expect(text()).toContain("think:low");

  f.setUsageResult({ ...f.usageResult, code: 1 });
  await command("codex-usage");
  expect(text()).toContain("weekly unavailable");
  expect(text()).not.toContain("79%");
  f.setUsageResult({ ...f.usageResult, code: 0, stdout: JSON.stringify([{
    provider: "codex", usage: { secondary: { usedPercent: 21 }, updatedAt: new Date(Date.now() - 700000).toISOString() },
  }]) });
  await command("codex-usage");
  expect(text()).toContain("stale data");
  expect(exec).toHaveBeenCalledTimes(3);
  expect(exec.mock.calls[0][0]).toBe("codexbar");

  editor.handleInput("\x1b[200~한글 입력\nsecond line\x1b[201~");
  expect(editor.getExpandedText()).toBe("한글 입력\nsecond line");
  for (const width of [5, 10, 20, 40, 80, 180]) {
    const lines = [...render(width), ...editor.render(width)];
    expect(lines.every(line => visibleWidth(line) <= width)).toBe(true);
    expect(editor.render(width).join("")).toContain(CURSOR_MARKER);
  }
  editor.handleInput("\r");
  expect(sent.pop()).toBe("한글 입력\nsecond line");
  editor.setText("");
  editor.addToHistory("previous prompt");
  editor.handleInput("\x1b[A");
  expect(editor.getText()).toBe("previous prompt");
  editor.setAutocompleteProvider(new CombinedAutocompleteProvider([{ name: "help" }, { name: "hello" }]));
  editor.setText("/h");
  editor.handleInput("\t");
  await Bun.sleep(0);
  expect(editor.isShowingAutocomplete()).toBe(true);
  editor.handleInput("\x1b");
  expect(editor.isShowingAutocomplete()).toBe(false);
  editor.setText("/compact keep recent changes");
  editor.handleInput("\r");
  expect(sent.pop()).toBe("/compact keep recent changes");
  expect(ctx.compact).not.toHaveBeenCalled();
  editor.setText("/compact");
  editor.handleInput("\r");
  expect(ctx.compact).toHaveBeenCalledTimes(1);
  editor.setText("during compaction");
  editor.handleInput("\r");
  expect(sent.pop()).toBe("during compaction");
  await emit("session_compact_failed", { errorMessage: "fixture failure" });
  const followUp = mock(() => {});
  editor.onAction("app.message.followUp", followUp);
  editor.setText("follow up");
  // Pi defaults to Ctrl+Q on WSL and Alt+Enter elsewhere.
  const followUpKey = ["\x11", "\x1b[13;3u"].find(key => keybindings.matches(key, "app.message.followUp"));
  expect(followUpKey).toBeDefined();
  editor.handleInput(followUpKey!);
  expect(followUp).toHaveBeenCalledTimes(1);
  const escape = mock(() => {});
  editor.onEscape = escape;
  editor.handleInput("\x1b");
  expect(escape).toHaveBeenCalledTimes(1);
  expect(terminalInput?.("\x1b[98;6u")).toBeUndefined(); // Ctrl+Shift+B remains shell-owned.
  await emit("input", { text: "steer", source: "interactive", streamingBehavior: "steer" });
  await emit("before_agent_start", { messageType: "user", prompt: "fixture" });
  await emit("agent_start");
  await emit("agent_end");
  expect(providerCall).not.toHaveBeenCalled();
  expect(pi.sendUserMessage).not.toHaveBeenCalled();
  expect(ctx.ui.setWorkingMessage.mock.calls).toEqual([[]]); // End-of-run restores Pi's normal message.
  expect(existsSync(join(process.env.PI_CODING_AGENT_DIR!, "powerline-footer/inbox.jsonl"))).toBe(false);

  await command("fast", "on");
  await f.reload();
  expect(text()).toContain("fast");
  f.editor.handleInput("after reload");
  expect(f.editor.getText()).toBe("after reload");
  process.env.PI_SUBAGENT_CHILD = "1";
  await emit("model_select");
  expect(text()).not.toContain("fast");
  ctx.ui.setStatus("codex-usage", undefined);
  expect(text()).not.toContain("weekly");
  expect(text()).toContain("Other Model");
}), 30_000);

test("Powerline mouse click reaches the rendered character, as Pi's base editor does", () => withPowerline(async f => {
  const base = f.makeBaseEditor();
  base.setPaddingX(settings.editorPaddingX);
  base.focused = true;
  base.setText("abcdefghij");
  const clickCharacter = (editor: any) => {
    const inputRow = stripTerminalSequences(editor.render(40)[1]);
    const x = inputRow.indexOf("d");
    editor.handleMouse({ type: "click", button: "left", x, y: 1, width: 40, height: 3 });
    return { x, col: editor.getCursor().col };
  };
  expect(clickCharacter(base)).toEqual({ x: 4, col: 3 });
  f.editor.setText("abcdefghij");
  // 0.22.0 index.ts prefixes rendered rows, but inherits untranslated handleMouse.
  // This is a required regression assertion, not an expected failure or acceptance waiver.
  expect(clickCharacter(f.editor)).toEqual({ x: 7, col: 3 });
}), 30_000);
