import { CustomEditor, type ExtensionAPI } from "@earendil-works/pi-coding-agent";

const SOFTWARE_CURSOR_PATTERN = /\x1b_pi:\/?fc\x07/g;

export function stripSoftwareCursor(line: string): string {
  return line.replace(SOFTWARE_CURSOR_PATTERN, "");
}

class HardwareCursorOnlyEditor extends CustomEditor {
  private readonly dimBorder = this.borderColor;

  render(width: number): string[] {
    // Pi replaces borderColor when thinking or bash mode changes.
    this.borderColor = this.dimBorder;
    this.setPaddingX(Math.max(2, this.getPaddingX()));
    const lines = super.render(width).map(stripSoftwareCursor);
    if (width >= 3) lines[1] = `❯${lines[1].slice(1)}`;
    return lines;
  }
}

export default function (pi: ExtensionAPI) {
  pi.on("session_start", (_event, ctx) => {
    if (ctx.mode !== "tui") return;
    ctx.ui.setEditorComponent(
      (tui, theme, keybindings) => new HardwareCursorOnlyEditor(tui, {
        ...theme,
        borderColor: (text) => ctx.ui.theme.fg("dim", text),
      }, keybindings),
    );
  });
}
