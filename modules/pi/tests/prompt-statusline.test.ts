import { describe, expect, test } from "bun:test";
import { CURSOR_MARKER, visibleWidth } from "@earendil-works/pi-tui";
import { createStatusLine } from "../files/extensions/prompt-ui/statusline";

describe("prompt statusline", () => {
  function fixture() {
    const statuses = new Map([["codex-fast-mode", "off"], ["codex-usage", "---------- 9% (⏳ 3d 14h)"]]);
    const ctx = {
      model: { id: "test-model", provider: "openai", reasoning: true, contextWindow: 1_000_000 },
      thinkingLevel: "high",
      sessionManager: {
        getCwd: () => "/workspace/한글-project",
      },
      getContextUsage: () => ({ percent: 11, contextWindow: 1_000_000 }),
      ui: { theme: {
        colors: { userMessageBg: "#edf0f3" },
        style: (text: string, { fg }: { fg: string }) => {
          expect(fg).toBe("#edf0f3");
          return `\x1b[38;2;237;240;243m${text}\x1b[39m`;
        },
        fg: (_: string, text: string) => `\x1b[36m${text}\x1b[39m`,
        bg: (_: string, text: string) => `\x1b[47m${text}\x1b[49m`,
      } },
    };
    const data = { getGitBranch: () => "main", getExtensionStatuses: () => statuses };
    const render = createStatusLine(ctx as never, data as never);
    const line = (width = 300) => render(width).replace(/\x1b\[[0-9;]*m/g, "");
    return { statuses, ctx, data, render, line };
  }

  test("preserves every status and places usage immediately before the model", () => {
    const { statuses, line } = fixture();
    expect(line()).toStartWith("╭\uE0B6 /workspace/한글-project › main \uE0B0");
    expect(line()).toContain("11%");
    expect(line()).not.toContain("/bg");
    expect(line()).not.toContain("tokens");
    expect(line()).not.toContain("↓");
    expect(line()).toEndWith("---------- 9% (⏳ 3d 14h) │ test-model · high");
    statuses.set("codex-fast-mode", "on");
    statuses.set("custom", "other extension");
    expect(line()).toEndWith("test-model · high · fast");
    expect(line()).toContain("other extension");
    statuses.set("codex-usage", "weekly unavailable (/codex-usage)");
    expect(line()).toContain("weekly unavailable");
    statuses.clear();
    expect(line()).not.toContain("⏳");
    expect(line(80)).toContain("test-model");
    expect(line(80)).toContain("11%");
  });

  test("shows fast only when enabled and follows model changes", () => {
    const { statuses, ctx, line } = fixture();
    statuses.delete("codex-fast-mode");
    expect(line()).not.toContain("fast");
    statuses.set("codex-fast-mode", "off");
    expect(line()).toEndWith("test-model · high");
    statuses.set("codex-fast-mode", "on");
    expect(line()).toEndWith("test-model · high · fast");
    ctx.model = { ...ctx.model, id: "another-model", provider: "anthropic" };
    expect(line()).toContain("another-model");
    expect(line()).not.toContain("fast");
    ctx.model.provider = "openai";
    expect(line()).toEndWith("another-model · high · fast");
    statuses.set("codex-fast-mode", "off");
    expect(line()).toEndWith("another-model · high");
  });

  test.each([
    ["---------- 0% (⏳ unknown)", ""],
    ["#--------- 10% (⏳ unknown)", "#"],
    ["#######--- 79% (⏳ 3d 14h)", "#######"],
    ["########## 100% (⏳ unknown)", "##########"],
    ["weekly unavailable (/codex-usage)", ""],
  ])("colors only the filled quota cells green: %s", (usage, filled) => {
    const { statuses, render, line } = fixture();
    statuses.set("codex-usage", usage);
    expect(line()).toContain(usage);
    if (filled) {
      expect(render(300)).toContain(`\x1b[32m${filled}\x1b[0m${usage.slice(filled.length)}`);
    } else {
      expect(render(300)).not.toContain("\x1b[32m");
    }
  });

  test("colors effort purple and keeps fast orange", () => {
    const { statuses, render } = fixture();
    statuses.set("codex-fast-mode", "on");
    expect(render(300)).toContain("\x1b[35mhigh\x1b[0m");
    expect(render(300)).toContain("\x1b[1;33mfast\x1b[0m");
  });

  test("centers the percentage across the entire context meter", () => {
    const { ctx, line } = fixture();
    const usage = ctx.getContextUsage();
    ctx.getContextUsage = () => ({ ...usage, percent: 51 });
    for (const width of [160, 220, 300]) {
      const meter = /([━─]+)\s*51%(?: \/ \S+)?\s*([━─]+)/.exec(line(width));
      expect(meter).not.toBeNull();
      expect(Math.abs(visibleWidth(meter![1]) - visibleWidth(meter![2]))).toBeLessThanOrEqual(1);
    }
  });

  test("scales the filled line to the full meter width behind its centered label", () => {
    const { ctx, line } = fixture();
    const usage = ctx.getContextUsage();
    for (const percent of [0, 25, 51, 75, 100]) {
      ctx.getContextUsage = () => ({ ...usage, percent });
      const meter = /([━─]+) (\d+%(?: \/ \S+)?) ([━─]+)/.exec(line(300))!;
      expect(meter).not.toBeNull();
      const leftWidth = meter[1].length;
      const rightStart = leftWidth + meter[2].length + 2;
      const meterWidth = rightStart + meter[3].length;
      const filled = Math.floor(meterWidth * percent / 100);
      for (let i = 0; i < leftWidth; i++) expect(meter[1][i]).toBe(i < filled ? "━" : "─");
      for (let i = 0; i < meter[3].length; i++) expect(meter[3][i]).toBe(rightStart + i < filled ? "━" : "─");
    }
  });

  test("separates the meter from the right-hand statuses with a padded divider", () => {
    const { line } = fixture();
    for (const width of [80, 160, 220, 300]) {
      expect(line(width)).toMatch(/[━─%M] │ (?:---------- 9%|test-model)/);
    }
  });

  test("shows the effective context limit, including before usage is known", () => {
    const { ctx, line } = fixture();
    expect(line()).toContain("11% / 1M");
    expect(line(80)).toContain("11% / 1M");
    ctx.getContextUsage = () => ({ percent: 25, contextWindow: 272_000 });
    expect(line()).toContain("25% / 272K");
    ctx.getContextUsage = () => undefined as never;
    expect(line()).toContain("?% / 1M");
    ctx.model.contextWindow = 1_050_000;
    expect(line()).toContain("?% / 1.05M");
    ctx.model = undefined as never;
    expect(line()).toContain("?%");
    expect(line()).not.toContain("?% /");
  });

  test("bounds real ANSI and wide text at every width, including selected tasks", () => {
    const { statuses, render } = fixture();
    statuses.set("codex-usage", "#######--- 79% (⏳ 3d 14h)");
    for (const selected of [false, true]) {
      if (selected) statuses.set("bg-selection", "selected");
      for (let width = 0; width <= 320; width++) {
        expect(visibleWidth(render(width))).toBeLessThanOrEqual(width);
        expect(render(width)).not.toContain(CURSOR_MARKER);
      }
    }
  });

  test("leaves background task display and selection to the input row", () => {
    const { statuses, line } = fixture();
    statuses.set("bg", "bg: 3");
    statuses.set("bg-selection", "selected");
    expect(line()).not.toContain("/bg");
    expect(line()).not.toContain("bg: 3");
    expect(line()).not.toContain("selected");
    expect(line()).not.toContain(CURSOR_MARKER);
  });

  test("keeps scroll hints and handles missing context, long branches, and theme changes", () => {
    const { ctx, data, render } = fixture();
    expect(render(80, 7)).toContain("↑ 7 more");
    ctx.getContextUsage = () => undefined as never;
    data.getGitBranch = () => "한글".repeat(100);
    for (const width of [4, 20, 60, 100, 200]) expect(visibleWidth(render(width))).toBeLessThanOrEqual(width);
    expect(render(80)).toContain("?%");
    ctx.ui.theme.fg = (_, text) => `\x1b[31m${text}\x1b[39m`;
    expect(render(80)).toContain("\x1b[31mtest-model");
  });
});
