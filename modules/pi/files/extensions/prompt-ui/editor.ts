import { CustomEditor } from "@earendil-works/pi-coding-agent";
import { CURSOR_MARKER } from "@earendil-works/pi-tui";

const SOFTWARE_CURSOR_PATTERN = /\x1b_pi:\/?fc\x07/g;

export function stripSoftwareCursor(line: string): string {
  return line.replace(SOFTWARE_CURSOR_PATTERN, "");
}

export class PromptEditor extends CustomEditor {
  private readonly promptBorder = this.borderColor;
  renderStatusLine?: (width: number, hiddenLines: number) => string;

  protected renderTopBorder(width: number, hiddenLines: number): string {
    const line = this.renderStatusLine?.(width, hiddenLines) ?? super.renderTopBorder(width, hiddenLines);
    return this.focused ? line : line.replaceAll(CURSOR_MARKER, "");
  }

  protected renderBottomBorder(width: number, hiddenLines: number): string {
    return hiddenLines > 0 ? super.renderBottomBorder(width, hiddenLines) : "";
  }

  render(width: number): string[] {
    if (width <= 0) return [];
    // Pi replaces borderColor when thinking or bash mode changes.
    this.borderColor = this.promptBorder;
    this.setPaddingX(Math.max(3, this.getPaddingX()));
    const lines = super.render(width).map(stripSoftwareCursor);
    const promptWidth = width >= 5 ? 2 : width >= 3 ? 1 : 0;
    if (promptWidth) lines[1] = this.promptBorder("╰─".slice(0, promptWidth)) + lines[1].slice(promptWidth);
    // Keep the autocomplete separator in place so built-in mouse rows still match.
    if (lines.at(-1) === "") lines.pop();
    return lines;
  }
}
