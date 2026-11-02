import { describe, expect, test } from "bun:test";
import { CustomEditor } from "@earendil-works/pi-coding-agent";
import { CombinedAutocompleteProvider, CURSOR_MARKER, visibleWidth } from "@earendil-works/pi-tui";
import hardwareCursorOnly, { stripSoftwareCursor } from "../files/extensions/hardware-cursor-only";

const dim = (text: string) => `\x1b[2m${text}\x1b[22m`;
const plain = (text: string) => text;
const theme = {
  borderColor: dim,
  selectList: { selectedText: plain, description: plain, scrollInfo: plain, noMatch: plain },
};
const tui = { terminal: { rows: 24 }, requestRender() {} };
const keybindings = { matches: () => false };

function createEditor(mode = "tui") {
  let editor: CustomEditor | undefined;
  hardwareCursorOnly({
    on: (_event: string, handler: Function) => handler({}, {
      mode,
      ui: {
        theme: { fg: (_color: string, text: string) => dim(text) },
        setEditorComponent: (factory: Function) => {
          editor = factory(tui, theme, keybindings);
        },
      },
    }),
  } as never);
  if (editor) editor.focused = true;
  return editor;
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

  test("leaves non-TUI modes alone", () => {
    expect(createEditor("rpc")).toBeUndefined();
    expect(createEditor("print")).toBeUndefined();
  });
});
