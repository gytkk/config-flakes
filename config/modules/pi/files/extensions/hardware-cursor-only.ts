import { CustomEditor, type ExtensionAPI } from "@earendil-works/pi-coding-agent";

const SOFTWARE_CURSOR_PATTERN = /\x1b\[7m([^\x1b]*)\x1b\[0m/g;

export function stripSoftwareCursor(line: string): string {
  return line.replace(SOFTWARE_CURSOR_PATTERN, "$1");
}

class HardwareCursorOnlyEditor extends CustomEditor {
  render(width: number): string[] {
    return super.render(width).map(stripSoftwareCursor);
  }
}

export default function (pi: ExtensionAPI) {
  pi.on("session_start", (_event, ctx) => {
    if (ctx.mode !== "tui") return;
    ctx.ui.setEditorComponent(
      (tui, theme, keybindings) => new HardwareCursorOnlyEditor(tui, theme, keybindings),
    );
  });
}
