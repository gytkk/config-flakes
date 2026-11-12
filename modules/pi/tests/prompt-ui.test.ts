import { describe, expect, mock, test } from "bun:test";
import { CustomEditor } from "@earendil-works/pi-coding-agent";
import { CombinedAutocompleteProvider, CURSOR_MARKER, matchesKey, visibleWidth } from "@earendil-works/pi-tui";
import promptUI from "../files/extensions/prompt-ui";
import fastMode from "../files/extensions/codex-fast-mode";
import { stripSoftwareCursor } from "../files/extensions/prompt-ui/editor";

const dim = (text: string) => `\x1b[2m${text}\x1b[22m`;
const plain = (text: string) => text;
const theme = {
  borderColor: dim,
  selectList: { selectedText: plain, description: plain, scrollInfo: plain, noMatch: plain },
};
const tui = { terminal: { rows: 24 }, requestRender() {} };
const keys: Record<string, string[]> = {
  "tui.editor.cursorDown": ["down"],
  "tui.editor.cursorUp": ["up"],
  "tui.select.confirm": ["enter"],
  "tui.select.cancel": ["escape", "ctrl+c"],
};
const keybindings = {
  matches: (data: string, action: string) => (keys[action] ?? []).some((key) => matchesKey(data, key as never)),
};

function createEditor(mode = "tui", commands: string[] = [], fastOrder?: "before" | "after") {
  let editor: CustomEditor | undefined;
  const statuses = new Map<string, string>();
  let footer: { render: (width: number) => string[]; dispose: () => void } | undefined;
  let unsubscribed = false;
  const handlers: Function[] = [];
  let fastCommand: Function | undefined;
  const ctx = {
    mode,
    model: { id: "test-model", provider: "openai" },
    sessionManager: { getCwd: () => "/workspace", getBranch: () => [] },
    getContextUsage: () => ({ percent: 11 }),
    ui: {
      notify: mock(() => {}),
      setFooter: (factory: Function) => {
        footer = factory(tui, {}, {
          getGitBranch: () => "main", getExtensionStatuses: () => statuses,
          onBranchChange: () => () => { unsubscribed = true; },
        });
      },
      setStatus: (key: string, value: string | undefined) => {
        if (value === undefined) statuses.delete(key);
        else statuses.set(key, value);
      },
      theme: {
        colors: { userMessageBg: "#edf0f3" }, style: (text: string) => text,
        fg: (_color: string, text: string) => dim(text), bg: (_color: string, text: string) => text,
      },
      setEditorComponent: (factory: Function) => {
        editor = factory(tui, theme, keybindings);
      },
    },
  };
  const pi = {
    exec: async () => ({ stdout: "", stderr: "", code: 0, killed: false }),
    getCommands: () => commands.map((name) => ({ name })),
    on: (event: string, handler: Function) => { if (event === "session_start") handlers.push(handler); },
    registerCommand: (_name: string, command: { handler: Function }) => { fastCommand = command.handler; },
    appendEntry: mock(() => {}),
  };
  if (fastOrder === "before") fastMode(pi as never);
  promptUI(pi as never);
  if (fastOrder === "after") fastMode(pi as never);
  for (const handler of handlers) handler({}, ctx);
  if (editor) editor.focused = true;
  return editor ? Object.assign(editor, {
    statuses, footer, isUnsubscribed: () => unsubscribed,
    fast: (action: string) => fastCommand!(action, ctx),
  }) : undefined;
}

describe("prompt UI", () => {
  test.each(["before", "after"] as const)("reads fast mode statuses with the provider loaded %s the UI", async (order) => {
    const editor = createEditor("tui", [], order)!;
    expect(editor.render(180)[0]).not.toContain("fast");
    await editor.fast("on");
    expect(editor.render(180)[0]).toContain("\x1b[1;33mfast\x1b[0m");
    await editor.fast("off");
    expect(editor.render(180)[0]).not.toContain("fast");
    expect(editor.footer!.render(180)).toEqual([]);
  });

  test("works without fast mode or usage providers", () => {
    const editor = createEditor()!;
    const line = editor.render(180)[0];
    expect(line).toContain("test-model");
    expect(line).toContain("11%");
    expect(line).not.toContain("fast");
    expect(line).not.toContain("weekly");
  });

  test("shows a status border and open prompt with no duplicate footer", () => {
    const editor = createEditor()!;
    editor.borderColor = (text) => `purple:${text}`;
    const lines = editor.render(80);
    expect(lines).toHaveLength(2);
    expect(lines[0]).toContain("test-model");
    expect(lines[0]).toContain("11%");
    expect(lines[1]).toStartWith(`${dim("╰─")} ${CURSOR_MARKER} `);
    expect(lines.join("")).not.toContain("\x1b[7m");
    expect(lines.every((line) => visibleWidth(line) === 80)).toBe(true);
    expect(editor.footer!.render(80)).toEqual([]);
    editor.footer!.dispose();
    expect(editor.isUnsubscribed()).toBe(true);
  });

  test("preserves built-in wrapping, cursor placement, navigation, and submission", () => {
    const editor = createEditor()!;
    const original = new CustomEditor(tui as never, theme as never, keybindings as never);
    original.focused = true;
    original.setPaddingX(3);
    for (const current of [editor, original]) {
      current.setText("one two three four\n한글 text");
      current.render(12);
      current.handleInput("\x1b[A");
      current.handleInput("\x1b[D");
      current.handleInput("!");
    }
    const expected = original.render(12).map(stripSoftwareCursor);
    expected[1] = dim("╰─") + expected[1].slice(2);
    expected.pop();
    expect(editor.render(12).slice(1)).toEqual(expected.slice(1));
    expect(editor.getCursor()).toEqual(original.getCursor());
    for (const current of [editor, original]) {
      current.handleMouse({ type: "click", button: "left", x: 3, y: 1, width: 12 } as never);
    }
    expect(editor.getCursor()).toEqual(original.getCursor());
    let submitted: string | undefined;
    editor.onSubmit = (text) => { submitted = text; };
    const text = editor.getText();
    editor.handleInput("\r");
    expect(submitted).toBe(text);
    expect(submitted).not.toContain("╰─");
  });

  test("keeps autocomplete rows and mouse targets below a blank separator", async () => {
    const editor = createEditor()!;
    editor.setAutocompleteProvider(new CombinedAutocompleteProvider([{ name: "help" }]));
    editor.setText("/h");
    editor.handleInput("\t");
    await Bun.sleep(0);
    expect(editor.isShowingAutocomplete()).toBe(true);
    const lines = editor.render(20);
    expect(lines[1]).toStartWith(dim("╰─") + " /h");
    expect(lines[2]).toBe("");
    expect(lines[3]).toContain("help");
    expect(lines[3]).not.toContain("╰─");
    expect(lines.every((line) => visibleWidth(line) <= 20)).toBe(true);
    const result = editor.handleMouse({ type: "click", button: "left", x: 3, y: 3, width: 20 } as never);
    expect(result?.handled).toBe(true);
  });

  test("keeps multiline paste content and places the prompt only on the first visible row", () => {
    const editor = createEditor()!;
    const text = Array.from({ length: 12 }, (_, i) => `line ${i}`).join("\n");
    editor.handleInput(`\x1b[200~${text}\x1b[201~`);
    expect(editor.getExpandedText()).toBe(text);
    editor.setText(text);
    const lines = editor.render(20);
    expect(lines[1]).toStartWith(dim("╰─"));
    expect(lines.join("").split("╰─")).toHaveLength(2);
    expect(editor.render(80)[0]).toContain("more");
    expect(lines.join("")).toContain(CURSOR_MARKER);
  });

  test("fits narrow terminals without overwriting input", () => {
    for (const width of [1, 2, 3, 4, 5, 10]) {
      const editor = createEditor()!;
      if (width >= 3) editor.setText("abcde");
      const lines = editor.render(width);
      expect(lines.every((line) => visibleWidth(line) <= width)).toBe(true);
      expect(lines[1].includes("╰")).toBe(width >= 3);
      expect(editor.getText()).toBe(width >= 3 ? "abcde" : "");
    }
  });

  test("Down and Enter retain native input behavior even with a legacy bg status", () => {
    const editor = createEditor("tui", ["bg"])!;
    editor.statuses.set("bg", "bg: 1");
    const submitted: string[] = [];
    editor.onSubmit = (text) => { submitted.push(text); };
    editor.handleInput("\x1b[B");
    editor.handleInput("\r");
    expect(submitted).not.toContain("/bg");
    expect(editor.statuses.has("bg-selection")).toBe(false);
    expect(editor.render(80)[1]).not.toContain("/bg");
    expect(editor.render(80)[1]).toContain(CURSOR_MARKER);
  });

  test("preserves multiline movement, history, and autocomplete", async () => {
    const editor = createEditor()!;
    editor.addToHistory("previous prompt");
    editor.handleInput("\x1b[A");
    expect(editor.getText()).toBe("previous prompt");
    editor.handleInput("\x1b[B");
    expect(editor.getText()).toBe("");
    editor.setText("first\nsecond");
    editor.handleInput("\x1b[A");
    editor.handleInput("\x1b[B");
    expect(editor.getCursor().line).toBe(1);
    editor.setAutocompleteProvider(new CombinedAutocompleteProvider([{ name: "help" }, { name: "history" }]));
    editor.setText("/h");
    editor.handleInput("\t");
    await Bun.sleep(0);
    expect(editor.isShowingAutocomplete()).toBe(true);
    editor.handleInput("\x1b[B");
    expect(editor.isShowingAutocomplete()).toBe(true);
  });

  test("leaves non-TUI modes alone", () => {
    expect(createEditor("rpc")).toBeUndefined();
    expect(createEditor("print")).toBeUndefined();
  });
});
