import type { ExtensionContext } from "@earendil-works/pi-coding-agent";
import { ScrollView, Text, truncateToWidth } from "@earendil-works/pi-tui";

export async function showPage(ctx: ExtensionContext, title: string, body: string, hasNext: boolean, hasPrevious: boolean, signal: AbortSignal): Promise<"next" | "previous" | "back"> {
  if (signal.aborted || ctx.mode !== "tui") return "back";
  return ctx.ui.custom<"next" | "previous" | "back">((tui, theme, keys, done) => {
    const text = new Text(body, 0, 0);
    const scroll = new ScrollView(text, { scrollbar: "hidden" });
    const abort = () => done("back");
    signal.addEventListener("abort", abort, { once: true });
    if (signal.aborted) queueMicrotask(abort);
    return {
      render(width) {
        if (width < 1) return [];
        const lines = scroll.render(width);
        const height = Math.max(1, Math.min(24, tui.terminal.rows - 6));
        scroll.updateLayout(lines.length, height, () => tui.requestRender());
        return [
          truncateToWidth(theme.fg("accent", title), width),
          ...lines.slice(scroll.scrollTop, scroll.scrollTop + height).map(line => truncateToWidth(line, width)),
          truncateToWidth(theme.fg("muted", `↑↓ / PgUp/PgDn scroll · ${hasNext ? "n next page · " : ""}${hasPrevious ? "p previous page · " : ""}Esc list`), width),
        ];
      },
      handleInput(data) {
        if (keys.matches(data, "tui.select.cancel")) { done("back"); return; }
        if (data === "n" && hasNext) { done("next"); return; }
        if (data === "p" && hasPrevious) { done("previous"); return; }
        if (keys.matches(data, "tui.select.up")) scroll.scrollBy(-1);
        else if (keys.matches(data, "tui.select.down")) scroll.scrollBy(1);
        else if (keys.matches(data, "tui.select.pageUp")) scroll.scrollBy(-Math.max(1, scroll.viewportHeight));
        else if (keys.matches(data, "tui.select.pageDown")) scroll.scrollBy(Math.max(1, scroll.viewportHeight));
        tui.requestRender();
      },
      invalidate() { text.invalidate(); scroll.invalidate(); },
      dispose() { signal.removeEventListener("abort", abort); },
    };
  });
}
