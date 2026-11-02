import { CustomEditor, type ExtensionAPI, type KeybindingsManager } from "@earendil-works/pi-coding-agent";
import { CURSOR_MARKER, type TuiMouseEvent } from "@earendil-works/pi-tui";

const SOFTWARE_CURSOR_PATTERN = /\x1b_pi:\/?fc\x07/g;

export function stripSoftwareCursor(line: string): string {
  return line.replace(SOFTWARE_CURSOR_PATTERN, "");
}

class HardwareCursorOnlyEditor extends CustomEditor {
  private readonly dimBorder = this.borderColor;
  private backgroundSelected = false;
  hasBackgroundCommand = () => false;
  matchesBackgroundKey: KeybindingsManager["matches"] = () => false;
  onBackgroundSelectionChange = (_selected: boolean) => {};

  private setBackgroundSelected(selected: boolean): void {
    if (this.backgroundSelected === selected) return;
    this.backgroundSelected = selected;
    this.onBackgroundSelectionChange(selected);
    this.tui.requestRender();
  }

  private showBackgroundItem(): boolean {
    return this.getText() === "" && !this.isShowingAutocomplete() && this.hasBackgroundCommand();
  }

  setText(text: string): void {
    this.setBackgroundSelected(false);
    super.setText(text);
  }

  handleMouse(event: TuiMouseEvent) {
    if (event.type === "click") this.setBackgroundSelected(false);
    return super.handleMouse(event);
  }

  handleInput(data: string): void {
    if (this.showBackgroundItem()) {
      if (this.matchesBackgroundKey(data, "tui.editor.cursorDown")) {
        this.setBackgroundSelected(true);
        return;
      }
      if (this.backgroundSelected) {
        this.setBackgroundSelected(false);
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
    this.setBackgroundSelected(false);
    super.handleInput(data);
  }

  render(width: number): string[] {
    // Pi replaces borderColor when thinking or bash mode changes.
    this.borderColor = this.dimBorder;
    this.setPaddingX(Math.max(2, this.getPaddingX()));
    if (!this.showBackgroundItem()) this.setBackgroundSelected(false);
    const lines = super.render(width).map(stripSoftwareCursor);
    if (width >= 3) lines[1] = `❯${lines[1].slice(1)}`;
    if (this.backgroundSelected) {
      for (let i = 0; i < lines.length; i++) lines[i] = lines[i].replaceAll(CURSOR_MARKER, "");
    }
    return lines;
  }
}

export default function (pi: ExtensionAPI) {
  pi.on("session_start", (_event, ctx) => {
    if (ctx.mode !== "tui") return;
    ctx.ui.setStatus("bg-selection", undefined);
    ctx.ui.setEditorComponent((tui, theme, keybindings) => {
      const editor = new HardwareCursorOnlyEditor(tui, {
        ...theme,
        borderColor: (text) => ctx.ui.theme.fg("dim", text),
      }, keybindings);
      editor.matchesBackgroundKey = (data, action) => keybindings.matches(data, action);
      editor.hasBackgroundCommand = () => pi.getCommands().some((command) => command.name === "bg");
      editor.onBackgroundSelectionChange = (selected) => ctx.ui.setStatus("bg-selection", selected ? "selected" : undefined);
      return editor;
    });
  });
}
