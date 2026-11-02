import { CustomEditor, type ExtensionAPI, type KeybindingsManager } from "@earendil-works/pi-coding-agent";
import { CURSOR_MARKER, truncateToWidth, type TuiMouseEvent } from "@earendil-works/pi-tui";

const SOFTWARE_CURSOR_PATTERN = /\x1b_pi:\/?fc\x07/g;

export function stripSoftwareCursor(line: string): string {
  return line.replace(SOFTWARE_CURSOR_PATTERN, "");
}

class HardwareCursorOnlyEditor extends CustomEditor {
  private readonly dimBorder = this.borderColor;
  private backgroundSelected = false;
  hasBackgroundCommand = () => false;
  matchesBackgroundKey: KeybindingsManager["matches"] = () => false;
  styleBackgroundItem = (text: string, _selected: boolean) => text;

  private showBackgroundItem(): boolean {
    return this.getText() === "" && !this.isShowingAutocomplete() && this.hasBackgroundCommand();
  }

  setText(text: string): void {
    this.backgroundSelected = false;
    super.setText(text);
  }

  handleMouse(event: TuiMouseEvent) {
    if (event.type === "click") this.backgroundSelected = false;
    return super.handleMouse(event);
  }

  handleInput(data: string): void {
    if (this.showBackgroundItem()) {
      if (this.matchesBackgroundKey(data, "tui.editor.cursorDown")) {
        this.backgroundSelected = true;
        this.tui.requestRender();
        return;
      }
      if (this.backgroundSelected) {
        this.backgroundSelected = false;
        this.tui.requestRender();
        if (this.matchesBackgroundKey(data, "tui.select.confirm")) {
          this.onSubmit?.("/bg");
          return;
        }
        if (
          this.matchesBackgroundKey(data, "tui.editor.cursorUp") ||
          this.matchesBackgroundKey(data, "tui.select.cancel")
        ) return;
      }
    }
    this.backgroundSelected = false;
    super.handleInput(data);
  }

  render(width: number): string[] {
    // Pi replaces borderColor when thinking or bash mode changes.
    this.borderColor = this.dimBorder;
    this.setPaddingX(Math.max(2, this.getPaddingX()));
    const showBackground = this.showBackgroundItem();
    if (!showBackground) this.backgroundSelected = false;
    const lines = super.render(width).map(stripSoftwareCursor);
    if (width >= 3) lines[1] = `❯${lines[1].slice(1)}`;
    if (showBackground) {
      const selected = this.backgroundSelected;
      const label = selected
        ? "> Background tasks (/bg)  Enter: open  Up/Esc: back"
        : "  Background tasks (/bg)  Down: select";
      if (selected) {
        for (let i = 0; i < lines.length; i++) lines[i] = lines[i].replaceAll(CURSOR_MARKER, "");
      }
      const cursor = selected && this.focused ? CURSOR_MARKER : "";
      lines.push(cursor + this.styleBackgroundItem(truncateToWidth(label, width), selected));
    }
    return lines;
  }
}

export default function (pi: ExtensionAPI) {
  pi.on("session_start", (_event, ctx) => {
    if (ctx.mode !== "tui") return;
    ctx.ui.setEditorComponent((tui, theme, keybindings) => {
      const editor = new HardwareCursorOnlyEditor(tui, {
        ...theme,
        borderColor: (text) => ctx.ui.theme.fg("dim", text),
      }, keybindings);
      editor.matchesBackgroundKey = (data, action) => keybindings.matches(data, action);
      editor.hasBackgroundCommand = () => pi.getCommands().some((command) => command.name === "bg");
      editor.styleBackgroundItem = (text, selected) => ctx.ui.theme.fg(selected ? "accent" : "dim", text);
      return editor;
    });
  });
}
