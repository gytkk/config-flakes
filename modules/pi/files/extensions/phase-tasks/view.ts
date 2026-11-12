import type { Theme } from "@earendil-works/pi-coding-agent";
import { ScrollView, Text, truncateToWidth, visibleWidth, type Component, type KeybindingsManager } from "@earendil-works/pi-tui";
import { currentTask, treeText, type TaskState } from "./state";

export function widgetLine(state: TaskState, width: number): string | undefined {
  const current = currentTask(state);
  if (!current || width < 1) return undefined;
  const count = `${current.remaining} left`;
  if (width <= visibleWidth(count) + 3) return truncateToWidth(count, width);
  const label = `${current.task.status === "pending" ? "Next" : "Doing"}: ${current.phase.name} / ${current.task.name}`;
  return `${truncateToWidth(label, width - visibleWidth(count) - 3)} · ${count}`;
}

/** Native wrapping + scrolling keeps every task reachable, even in narrow panes. */
export function createTaskViewer(
  state: TaskState, theme: Theme, keys: KeybindingsManager,
  rows: () => number, requestRender: () => void, done: () => void,
  warning?: string,
): Component {
  const text = new Text(warning ?? treeText(state), 0, 0);
  const scroll = new ScrollView(text, { scrollbar: "hidden" });
  return {
    render(width) {
      if (width < 1) return [];
      const height = Math.max(1, Math.min(24, rows() - 4));
      const lines = scroll.render(width);
      scroll.updateLayout(lines.length, height, requestRender);
      return [
        truncateToWidth(theme.fg("accent", "Tasks · current branch"), width),
        ...lines.slice(scroll.scrollTop, scroll.scrollTop + height).map(line => truncateToWidth(line, width)),
        truncateToWidth(theme.fg("muted", `${scroll.scrollTop + 1}–${Math.min(lines.length, scroll.scrollTop + height)}/${lines.length} · ↑↓ PgUp/PgDn scroll · Esc close`), width),
      ];
    },
    handleInput(data) {
      if (keys.matches(data, "tui.select.cancel") || keys.matches(data, "tui.select.confirm")) { done(); return; }
      if (keys.matches(data, "tui.select.up")) scroll.scrollBy(-1);
      else if (keys.matches(data, "tui.select.down")) scroll.scrollBy(1);
      else if (keys.matches(data, "tui.select.pageUp")) scroll.scrollBy(-Math.max(1, scroll.viewportHeight));
      else if (keys.matches(data, "tui.select.pageDown")) scroll.scrollBy(Math.max(1, scroll.viewportHeight));
      requestRender();
    },
    invalidate() { text.invalidate(); scroll.invalidate(); },
  };
}
