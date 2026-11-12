import { CustomEditor, type ExtensionAPI, type KeybindingsManager } from "@earendil-works/pi-coding-agent";
import { CURSOR_MARKER, truncateToWidth, visibleWidth, type TuiMouseEvent } from "@earendil-works/pi-tui";
import { createStatusLine } from "./codex-fast-mode";

const SOFTWARE_CURSOR_PATTERN = /\x1b_pi:\/?fc\x07/g;

export function stripSoftwareCursor(line: string): string {
  return line.replace(SOFTWARE_CURSOR_PATTERN, "");
}

class HardwareCursorOnlyEditor extends CustomEditor {
  private readonly dimBorder = this.borderColor;
  private backgroundSelected = false;
  renderStatusLine?: (width: number, hiddenLines: number) => string;
  hasBackgroundCommand = () => false;
  getBackgroundStatus = (): string | undefined => undefined;
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

  protected renderTopBorder(width: number, hiddenLines: number): string {
    const line = this.renderStatusLine?.(width, hiddenLines) ?? super.renderTopBorder(width, hiddenLines);
    return this.focused ? line : line.replaceAll(CURSOR_MARKER, "");
  }

  protected renderBottomBorder(width: number, hiddenLines: number): string {
    return hiddenLines > 0 ? super.renderBottomBorder(width, hiddenLines) : "";
  }

  private renderBackgroundItem(line: string, width: number): string {
    if (!this.hasBackgroundCommand() || this.isShowingAutocomplete()) return line;
    const status = this.getBackgroundStatus();
    const count = status === undefined ? "0" : /^bg: (\d+)$/.exec(status)?.[1] ?? "?";
    const label = `${this.backgroundSelected ? "* " : ""}/bg (${count})`;
    const marker = this.backgroundSelected && this.focused ? CURSOR_MARKER : "";
    const style = (text: string) => this.backgroundSelected ? `\x1b[1;35m${text}\x1b[0m` : this.dimBorder(text);
    const inputWidth = width - visibleWidth(label) - 1;
    const content = line.trimEnd();
    // An end-of-line cursor occupies a blank cell that trimEnd removes.
    const occupied = visibleWidth(content) + (content.endsWith(CURSOR_MARKER) ? 1 : 0);
    if (inputWidth < occupied) {
      if (!this.backgroundSelected) return line;
      const clipped = truncateToWidth(label, width, "");
      return " ".repeat(width - visibleWidth(clipped)) + marker + style(clipped);
    }
    return truncateToWidth(line, inputWidth, "") + " " + marker + style(label);
  }

  render(width: number): string[] {
    if (width <= 0) return [];
    // Pi replaces borderColor when thinking or bash mode changes.
    this.borderColor = this.dimBorder;
    this.setPaddingX(Math.max(2, this.getPaddingX()));
    if (!this.showBackgroundItem()) this.setBackgroundSelected(false);
    const lines = super.render(width).map(stripSoftwareCursor);
    const promptWidth = width >= 5 ? 2 : width >= 3 ? 1 : 0;
    if (promptWidth) lines[1] = this.dimBorder("╰─".slice(0, promptWidth)) + lines[1].slice(promptWidth);
    if (this.backgroundSelected) {
      for (let i = 1; i < lines.length; i++) lines[i] = lines[i].replaceAll(CURSOR_MARKER, "");
    }
    lines[1] = this.renderBackgroundItem(lines[1], width);
    // Keep the autocomplete separator in place so built-in mouse rows still match.
    if (lines.at(-1) === "") lines.pop();
    return lines;
  }
}

export default function (pi: ExtensionAPI) {
  pi.on("session_start", (_event, ctx) => {
    if (ctx.mode !== "tui") return;
    ctx.ui.setStatus("bg-selection", undefined);
    let renderStatusLine: ReturnType<typeof createStatusLine> | undefined;
    let getBackgroundStatus = (): string | undefined => undefined;
    ctx.ui.setFooter((tui, _theme, footerData) => {
      renderStatusLine = createStatusLine(ctx, footerData);
      getBackgroundStatus = () => footerData.getExtensionStatuses().get("bg");
      const unsubscribe = footerData.onBranchChange(() => tui.requestRender());
      return {
        render: () => [],
        invalidate() {},
        dispose: unsubscribe,
      };
    });
    ctx.ui.setEditorComponent((tui, theme, keybindings) => {
      const editor = new HardwareCursorOnlyEditor(tui, {
        ...theme,
        borderColor: (text) => ctx.ui.theme.fg("dim", text),
      }, keybindings);
      editor.renderStatusLine = (width, hiddenLines) => renderStatusLine?.(width, hiddenLines) ?? "";
      editor.getBackgroundStatus = () => getBackgroundStatus();
      editor.matchesBackgroundKey = (data, action) => keybindings.matches(data, action);
      editor.hasBackgroundCommand = () => pi.getCommands().some((command) => command.name === "bg");
      editor.onBackgroundSelectionChange = (selected) => ctx.ui.setStatus("bg-selection", selected ? "selected" : undefined);
      return editor;
    });
  });
}
