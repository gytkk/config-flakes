import { describe, expect, test } from "bun:test";
import { CustomEditor } from "@earendil-works/pi-coding-agent";
import { CombinedAutocompleteProvider, CURSOR_MARKER, matchesKey, visibleWidth } from "@earendil-works/pi-tui";
import hardwareCursorOnly, { stripSoftwareCursor } from "../files/extensions/hardware-cursor-only";

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

function createEditor(mode = "tui", commands: string[] = []) {
  let editor: CustomEditor | undefined;
  const statuses = new Map<string, string>();
  let footer: { render: (width: number) => string[]; dispose: () => void } | undefined;
  let unsubscribed = false;
  hardwareCursorOnly({
    getCommands: () => commands.map((name) => ({ name })),
    on: (_event: string, handler: Function) => handler({}, {
      mode,
      model: { id: "test-model", provider: "openai" },
      sessionManager: {
        getCwd: () => "/workspace",
      },
      getContextUsage: () => ({ percent: 11 }),
      ui: {
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
    }),
  } as never);
  if (editor) editor.focused = true;
  return editor ? Object.assign(editor, { statuses, footer, isUnsubscribed: () => unsubscribed }) : undefined;
}

describe("hardware cursor prompt", () => {
  test("shows a status border and open prompt with no duplicate footer", () => {
    const editor = createEditor()!;
    editor.borderColor = (text) => `purple:${text}`;
    const lines = editor.render(80);
    expect(lines).toHaveLength(2);
    expect(lines[0]).toContain("test-model");
    expect(lines[0]).toContain("11%");
    expect(lines[1]).toStartWith(`${dim("╰─")}${CURSOR_MARKER} `);
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
    original.setPaddingX(2);
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
    expect(lines[1]).toStartWith(dim("╰─") + "/h");
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

  test("right-aligns task counts on the first input row without changing text", () => {
    const editor = createEditor("tui", ["bg"])!;
    const plainLine = () => editor.render(80)[1].replace(/\x1b\[[0-9;]*m/g, "");
    expect(editor.render(80)[0]).not.toContain("/bg");
    expect(plainLine()).toEndWith(" /bg (0)");
    expect(visibleWidth(plainLine())).toBe(80);
    editor.statuses.set("bg", "bg: 3");
    expect(plainLine()).toEndWith(" /bg (3)");
    editor.statuses.set("bg", "invalid");
    expect(plainLine()).toEndWith(" /bg (?)");
    editor.statuses.delete("bg");
    editor.setText("한글\nsecond");
    expect(plainLine()).toEndWith(" /bg (0)");
    expect(editor.render(80)[2]).not.toContain("/bg");
    expect(editor.getText()).toBe("한글\nsecond");
  });

  test("hides task counts before overlapping text or the cursor without changing wrapping", () => {
    const editor = createEditor("tui", ["bg"])!;
    const original = createEditor()!;
    for (const text of ["abcdefghij", "한글abcdefgh", "abcdefghijklmnopqrs", "firstlinefillstheentirerow\nsecond"]) {
      editor.setText(text);
      original.setText(text);
      expect(editor.render(20)).toEqual(original.render(20));
      expect(editor.getCursor()).toEqual(original.getCursor());
    }
    editor.setText("abcdefghi");
    const line = editor.render(20)[1];
    expect(line).toContain("/bg (0)");
    expect(visibleWidth(line)).toBe(20);
    expect(visibleWidth(line.split(CURSOR_MARKER)[0])).toBe(11);
    editor.setText("");
    expect(editor.render(20)[1]).toContain("/bg (0)");
  });

  test("hides the right prompt while autocomplete owns navigation", async () => {
    const editor = createEditor("tui", ["bg"])!;
    editor.setAutocompleteProvider(new CombinedAutocompleteProvider([{ name: "bg" }, { name: "branch" }]));
    editor.setText("/b");
    editor.handleInput("\t");
    await Bun.sleep(0);
    expect(editor.isShowingAutocomplete()).toBe(true);
    expect(editor.render(80)[1]).not.toContain("/bg (");
    expect(editor.render(80)[1]).toContain(CURSOR_MARKER);
  });

  test("keeps the selected task cursor bounded at every width", () => {
    const editor = createEditor("tui", ["bg"])!;
    editor.handleInput("\x1b[B");
    for (let width = 1; width <= 160; width++) {
      const lines = editor.render(width);
      expect(lines.every((line) => visibleWidth(line) <= width)).toBe(true);
      expect(lines[0]).not.toContain(CURSOR_MARKER);
      expect(lines[1]).toContain(CURSOR_MARKER);
    }
  });

  test("selects background tasks with Down and submits /bg only on Enter", () => {
    const editor = createEditor("tui", ["bg"])!;
    const submitted: string[] = [];
    editor.onSubmit = (text) => { submitted.push(text); };
    expect(editor.render(80)).toHaveLength(2);
    editor.handleInput("\x1b[B");
    expect(submitted).toEqual([]);
    expect(editor.getText()).toBe("");
    const lines = editor.render(80);
    expect(lines).toHaveLength(2);
    expect(editor.statuses.get("bg-selection")).toBe("selected");
    expect(lines[0]).not.toContain(CURSOR_MARKER);
    expect(lines[1]).toContain(`${CURSOR_MARKER}\x1b[1;35m* /bg (0)`);
    expect(lines[1].split(CURSOR_MARKER)).toHaveLength(2);
    editor.focused = false;
    expect(editor.render(80).join("")).not.toContain(CURSOR_MARKER);
    editor.focused = true;
    editor.handleInput("\x1b[B");
    editor.handleInput("\r");
    expect(submitted).toEqual(["/bg"]);
    expect(editor.statuses.has("bg-selection")).toBe(false);
  });

  test("Up and Escape return to editing without submitting or aborting", () => {
    for (const key of ["\x1b[A", "\x1b", "\x03"]) {
      const editor = createEditor("tui", ["bg"])!;
      let submitted = false;
      editor.onSubmit = () => { submitted = true; };
      editor.onEscape = () => { throw new Error("selection must not abort the agent"); };
      editor.handleInput("\x1b[B");
      editor.handleInput(key);
      expect(editor.statuses.has("bg-selection")).toBe(false);
      expect(submitted).toBe(false);
    }
  });

  test("typing, paste, and programmatic text changes leave task selection", () => {
    for (const input of ["hello", "\x1b[200~한글\ntext\x1b[201~"]) {
      const editor = createEditor("tui", ["bg"])!;
      editor.handleInput("\x1b[B");
      editor.handleInput(input);
      expect(editor.render(80).join("")).not.toContain("Background tasks");
      expect(editor.getText()).not.toBe("");
      expect(editor.statuses.has("bg-selection")).toBe(false);
    }
    const editor = createEditor("tui", ["bg"])!;
    editor.handleInput("\x1b[B");
    editor.setText("");
    expect(editor.statuses.has("bg-selection")).toBe(false);
  });

  test("preserves multiline movement, history, and autocomplete", async () => {
    const editor = createEditor("tui", ["bg"])!;
    editor.addToHistory("previous prompt");
    editor.handleInput("\x1b[A");
    expect(editor.getText()).toBe("previous prompt");
    editor.handleInput("\x1b[B");
    expect(editor.getText()).toBe("");
    expect(editor.statuses.has("bg-selection")).toBe(false);
    editor.setText("first\nsecond");
    editor.handleInput("\x1b[A");
    editor.handleInput("\x1b[B");
    expect(editor.getCursor().line).toBe(1);
    expect(editor.render(80).join("")).not.toContain("Background tasks");
    editor.setAutocompleteProvider(new CombinedAutocompleteProvider([{ name: "bg" }, { name: "branch" }]));
    editor.setText("/b");
    editor.handleInput("\t");
    await Bun.sleep(0);
    expect(editor.isShowingAutocomplete()).toBe(true);
    editor.handleInput("\x1b[B");
    expect(editor.render(80).join("")).not.toContain("Background tasks");
  });

  test("clicking the input restores its cursor without opening tasks", () => {
    const editor = createEditor("tui", ["bg"])!;
    editor.handleInput("\x1b[B");
    editor.render(80);
    editor.handleMouse({ type: "click", button: "left", x: 3, y: 1, width: 80 } as never);
    expect(editor.statuses.has("bg-selection")).toBe(false);
    expect(editor.render(80)[1]).toContain(CURSOR_MARKER);
  });

  test("hides unavailable commands and fits selected rows to terminal width", () => {
    const commands = ["bg"];
    const editor = createEditor("tui", commands)!;
    editor.handleInput("\x1b[B");
    for (const width of [1, 2, 3, 20, 80]) {
      expect(editor.render(width).every((line) => visibleWidth(line) <= width)).toBe(true);
    }
    commands.length = 0;
    expect(editor.render(80).join("")).not.toContain("Background tasks");
    expect(editor.render(80).join("")).toContain(CURSOR_MARKER);
    expect(editor.statuses.has("bg-selection")).toBe(false);
  });

  test("leaves non-TUI modes alone", () => {
    expect(createEditor("rpc")).toBeUndefined();
    expect(createEditor("print")).toBeUndefined();
  });
});
