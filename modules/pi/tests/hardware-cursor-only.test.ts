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
  hardwareCursorOnly({
    getCommands: () => commands.map((name) => ({ name })),
    on: (_event: string, handler: Function) => handler({}, {
      mode,
      ui: {
        setStatus: (key: string, value: string | undefined) => {
          if (value === undefined) statuses.delete(key);
          else statuses.set(key, value);
        },
        theme: { fg: (_color: string, text: string) => dim(text) },
        setEditorComponent: (factory: Function) => {
          editor = factory(tui, theme, keybindings);
        },
      },
    }),
  } as never);
  if (editor) editor.focused = true;
  return editor ? Object.assign(editor, { statuses }) : undefined;
}

describe("hardware cursor prompt", () => {
  test("shows a fixed prompt and hardware cursor with dim borders", () => {
    const editor = createEditor()!;
    editor.borderColor = (text) => `purple:${text}`;
    const lines = editor.render(20);
    expect(lines[0]).toBe(dim("─".repeat(20)));
    expect(lines[1]).toStartWith(`❯ ${CURSOR_MARKER} `);
    expect(lines[2]).toBe(dim("─".repeat(20)));
    expect(lines.join("")).not.toContain("\x1b[7m");
    expect(lines.every((line) => visibleWidth(line) === 20)).toBe(true);
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
    expected[1] = `❯${expected[1].slice(1)}`;
    expect(editor.render(12)).toEqual(expected);
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
    expect(submitted).not.toContain("❯");
  });

  test("keeps autocomplete below the dim bottom border", async () => {
    const editor = createEditor()!;
    editor.setAutocompleteProvider(new CombinedAutocompleteProvider([{ name: "help" }]));
    editor.setText("/h");
    editor.handleInput("\t");
    await Bun.sleep(0);
    expect(editor.isShowingAutocomplete()).toBe(true);
    const lines = editor.render(20);
    expect(lines[1]).toStartWith("❯ /h");
    expect(lines[2]).toBe(dim("─".repeat(20)));
    expect(lines[3]).toContain("help");
    expect(lines[3]).not.toContain("❯");
    expect(lines.every((line) => visibleWidth(line) <= 20)).toBe(true);
  });

  test("keeps multiline paste content and places the prompt only on the first visible row", () => {
    const editor = createEditor()!;
    const text = Array.from({ length: 12 }, (_, i) => `line ${i}`).join("\n");
    editor.handleInput(`\x1b[200~${text}\x1b[201~`);
    expect(editor.getExpandedText()).toBe(text);
    editor.setText(text);
    const lines = editor.render(20);
    expect(lines[1]).toStartWith("❯ ");
    expect(lines.join("").split("❯")).toHaveLength(2);
    expect(lines[0]).toContain("more");
    expect(lines[0]).toStartWith("\x1b[2m");
    expect(lines.join("")).toContain(CURSOR_MARKER);
  });

  test("fits narrow terminals without overwriting input", () => {
    for (const width of [1, 2, 3, 4, 5, 10]) {
      const editor = createEditor()!;
      if (width >= 3) editor.setText("abcde");
      const lines = editor.render(width);
      expect(lines.every((line) => visibleWidth(line) <= width)).toBe(true);
      expect(lines[1].startsWith("❯")).toBe(width >= 3);
      expect(editor.getText()).toBe(width >= 3 ? "abcde" : "");
    }
  });

  test("selects background tasks with Down and submits /bg only on Enter", () => {
    const editor = createEditor("tui", ["bg"])!;
    const submitted: string[] = [];
    editor.onSubmit = (text) => { submitted.push(text); };
    expect(editor.render(80)).toHaveLength(3);
    editor.handleInput("\x1b[B");
    expect(submitted).toEqual([]);
    expect(editor.getText()).toBe("");
    const lines = editor.render(80);
    expect(lines).toHaveLength(3);
    expect(editor.statuses.get("bg-selection")).toBe("selected");
    expect(lines.join("")).not.toContain(CURSOR_MARKER);
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
