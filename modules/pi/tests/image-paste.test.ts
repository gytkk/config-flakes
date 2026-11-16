import { describe, expect, test } from "bun:test";
import { stripTerminalSequences, visibleWidth } from "@earendil-works/pi-tui";
import { PromptEditor } from "../files/extensions/prompt-ui/editor";

const plain = (text: string) => text;
const theme = { borderColor: plain, selectList: { selectedText: plain, description: plain, scrollInfo: plain, noMatch: plain } };
function editor() {
  const result = new PromptEditor({ terminal: { rows: 24 }, requestRender() {} } as never, theme, { matches: () => false } as never);
  result.focused = true;
  return result;
}
const paste = (target: PromptEditor, text: string) => target.handleInput(`\x1b[200~${text}\x1b[201~`);
const screenshot = "'/var/folders/test/T/herdr-drops/1794789109802-스크린샷 2026-11-16 오전 9.31.48.png'";

describe("image paste labels", () => {
  test("collapses a Herdr image path and submits the original reference", () => {
    const target = editor();
    paste(target, screenshot);
    expect(target.getText()).toBe("[Image #1]");
    const rendered = target.render(80).map(stripTerminalSequences).join("\n");
    expect(rendered).toContain("[Image #1]");
    expect(rendered).not.toContain("herdr-drops");
    expect(target.getExpandedText()).toBe(screenshot);
    let submitted = "";
    target.onSubmit = text => { submitted = text; };
    target.handleInput("\r");
    expect(submitted).toBe(screenshot);
    expect(target.getText()).toBe("");
    paste(target, screenshot);
    expect(target.getText()).toBe("[Image #1]");
  });

  test("supports native clipboard insertion, multiple images, and whitespace", () => {
    const target = editor();
    target.insertTextAtCursor(" /tmp/pi-clipboard-first.png\n/tmp/second image.JPEG ");
    expect(target.getText()).toBe(" [Image #1]\n[Image #2] ");
    expect(target.getExpandedText()).toBe(" /tmp/pi-clipboard-first.png\n/tmp/second image.JPEG ");
  });

  test("buffers split bracketed paste and preserves adjacent prompt text", () => {
    const target = editor();
    target.setText("Compare ");
    target.handleInput("\x1b[200~" + screenshot.slice(0, 20));
    target.handleInput(screenshot.slice(20) + "\x1b[201~ with ");
    paste(target, '"/tmp/second image.webp"');
    expect(target.getText()).toBe("Compare [Image #1] with [Image #2]");
    expect(target.getExpandedText()).toBe(`Compare ${screenshot} with "/tmp/second image.webp"`);
  });

  test("moves and deletes whole labels, with undo restoring their paths", () => {
    const target = editor();
    paste(target, screenshot);
    target.handleInput("\x1b[D");
    expect(target.getCursor().col).toBe(0);
    target.handleInput("\x1b[C");
    expect(target.getCursor().col).toBe(10);
    target.handleInput("\x7f");
    expect(target.getText()).toBe("");
    target.handleInput("\x1f");
    expect(target.getText()).toBe("[Image #1]");
    expect(target.getExpandedText()).toBe(screenshot);
  });

  test("keeps image and large-text markers distinct through renumbering", () => {
    const target = editor();
    const large = "long text\n".repeat(15);
    paste(target, screenshot);
    target.handleInput(" ");
    paste(target, large);
    target.handleInput("\x01");
    target.handleInput("\x1b[C");
    target.handleInput("\x7f");
    expect(target.getText()).toContain("[paste #1");
    expect(target.getText()).not.toContain("[Image");
    expect(target.getExpandedText()).toBe(" " + large);
    paste(target, screenshot);
    expect(target.getExpandedText()).toContain(screenshot);
    expect(target.getExpandedText()).toContain(large);
  });

  test("does not label prose, non-images, commands, or literal markers as images", () => {
    for (const text of ["explain /tmp/example.png", "/tmp/readme.md", "https://example.com/a.png", "[Image #1]", "echo /tmp/a.png", "'/tmp/a.png' '/tmp/b.png'"]) {
      const target = editor();
      paste(target, text);
      expect(target.getText()).toBe(text);
      expect(target.getExpandedText()).toBe(text);
    }
  });

  test("retains image labels when an earlier image is removed", () => {
    const target = editor();
    paste(target, "/tmp/first.png");
    target.handleInput(" ");
    paste(target, "/tmp/second.png");
    target.handleInput("\x01");
    target.handleInput("\x1b[C");
    target.handleInput("\x7f");
    expect(target.getText()).toBe(" [Image #1]");
    expect(target.getExpandedText()).toBe(" /tmp/second.png");
  });

  test("does not collapse paths in bash mode or corrupt large-paste undo", () => {
    const target = editor();
    target.setText("!cat ");
    paste(target, "/tmp/first.png");
    expect(target.getText()).toBe("!cat /tmp/first.png");
    target.setText("");
    const large = "word ".repeat(220);
    paste(target, large);
    target.insertTextAtCursor("/tmp/image.png");
    expect(target.getText()).toContain("[Image #2]");
    target.handleInput("\x1f");
    expect(target.getExpandedText()).toBe(large);
    target.insertTextAtCursor("/tmp/next.png");
    expect(target.getText()).toContain("[Image #2]");
    expect(target.getExpandedText()).toBe(large + "/tmp/next.png");
  });

  test("keeps narrow rendering and mouse coordinates aligned", () => {
    const target = editor();
    paste(target, screenshot);
    target.handleInput(" text");
    for (const width of [5, 12, 20, 80]) {
      expect(target.render(width).every(line => visibleWidth(line) <= width)).toBe(true);
    }
    target.render(80);
    target.handleMouse({ type: "click", button: "left", x: 15, y: 1, width: 80, height: 2 } as never);
    expect(target.getCursor().col).toBe(12);
    target.handleInput("!");
    expect(target.getText()).toBe("[Image #1] t!ext");
    expect(target.getExpandedText()).not.toContain("[Image");
    expect(target.getExpandedText()).toContain(screenshot);
  });
});
