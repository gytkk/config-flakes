import type { ExtensionContext, Theme } from "@earendil-works/pi-coding-agent";
import { ScrollView, Text, stripTerminalSequences, truncateToWidth, type Component, type KeybindingsManager } from "@earendil-works/pi-tui";

export function displayText(value: string, limit = 100_000): string {
  const bounded = value.length > limit ? `${value.slice(0, limit)}\n[Hub display truncated at ${limit} characters]` : value;
  return stripTerminalSequences(bounded).replace(/[\x00-\x08\x0b-\x1f\x7f-\x9f]/g, "");
}

// A read-only viewport composed from Pi's Text and ScrollView. No editor or
// child-session focus: closing it always returns to the owning parent.
export function createViewer(title: string, body: string, theme: Theme, keys: KeybindingsManager, rows: () => number, render: () => void, done: () => void): Component {
  const text = new Text(displayText(body), 0, 0);
  const scroll = new ScrollView(text, { scrollbar: "hidden" });
  return {
    render(width) {
      if (width < 1) return [];
      const lines = scroll.render(width);
      const height = Math.max(1, Math.min(24, rows() - 6));
      scroll.updateLayout(lines.length, height, render);
      return [
        truncateToWidth(theme.fg("accent", displayText(title, 500)), width),
        ...lines.slice(scroll.scrollTop, scroll.scrollTop + height).map(line => truncateToWidth(line, width)),
        truncateToWidth(theme.fg("muted", `${scroll.scrollTop + 1}–${Math.min(lines.length, scroll.scrollTop + height)}/${lines.length} · ↑↓ scroll · PgUp/PgDn · Esc back`), width),
      ];
    },
    handleInput(data) {
      if (keys.matches(data, "tui.select.cancel") || keys.matches(data, "tui.select.confirm")) { done(); return; }
      if (keys.matches(data, "tui.select.up")) scroll.scrollBy(-1);
      else if (keys.matches(data, "tui.select.down")) scroll.scrollBy(1);
      else if (keys.matches(data, "tui.select.pageUp")) scroll.scrollBy(-Math.max(1, scroll.viewportHeight));
      else if (keys.matches(data, "tui.select.pageDown")) scroll.scrollBy(Math.max(1, scroll.viewportHeight));
      render();
    },
    invalidate() { text.invalidate(); scroll.invalidate(); },
  };
}

export async function showText(ctx: ExtensionContext, title: string, body: string, signal: AbortSignal): Promise<void> {
  if (signal.aborted) return;
  await ctx.ui.custom<void>((tui, theme, keys, done) => {
    const abort = () => done();
    signal.addEventListener("abort", abort, { once: true });
    return {
      ...createViewer(title, body, theme, keys, () => tui.terminal.rows, () => tui.requestRender(), done),
      dispose() { signal.removeEventListener("abort", abort); },
    };
  });
}
