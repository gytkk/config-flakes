import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { PromptEditor } from "./editor";
import { createStatusLine } from "./statusline";
import { watchGitStatus } from "./git-status";

export default function (pi: ExtensionAPI) {
  let disposeFooter = () => {};
  pi.on("session_shutdown", () => disposeFooter());
  pi.on("session_start", (_event, ctx) => {
    disposeFooter();
    if (ctx.mode !== "tui") return;
    let renderStatusLine: ReturnType<typeof createStatusLine> | undefined;
    ctx.ui.setFooter((tui, _theme, footerData) => {
      let gitStatus = "";
      renderStatusLine = createStatusLine(ctx, footerData, () => gitStatus);
      const stopGitStatus = watchGitStatus(pi.exec.bind(pi), ctx.sessionManager.getCwd(), (status) => {
        if (status === gitStatus) return;
        gitStatus = status;
        tui.requestRender();
      }, (message) => ctx.ui.notify(message, "warning"));
      const unsubscribe = footerData.onBranchChange(() => tui.requestRender());
      let disposed = false;
      const dispose = () => {
        if (disposed) return;
        disposed = true;
        stopGitStatus();
        unsubscribe();
      };
      disposeFooter = dispose;
      return {
        render: () => [],
        invalidate() {},
        dispose,
      };
    });
    ctx.ui.setEditorComponent((tui, theme, keybindings) => {
      const editor = new PromptEditor(tui, {
        ...theme,
        borderColor: (text) => ctx.ui.theme.fg("accent", text),
      }, keybindings);
      editor.renderStatusLine = (width, hiddenLines) => renderStatusLine?.(width, hiddenLines) ?? "";
      return editor;
    });
  });
}
