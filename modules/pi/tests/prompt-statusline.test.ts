import { describe, expect, test } from "bun:test";
import { CURSOR_MARKER, colorToRgb, mixColors, stripTerminalSequences, visibleWidth } from "@earendil-works/pi-tui";
import { createStatusLine } from "../files/extensions/prompt-ui/statusline";
import { promptTheme } from "./fixtures/prompt-theme";

describe("prompt statusline", () => {
  function fixture(gitStatus = "") {
    const statuses = new Map([["codex-fast-mode", "off"], ["codex-usage", "---------- 9% (⏳ 3d 14h)"]]);
    const ctx = {
      model: { id: "test-model", provider: "openai", reasoning: true, contextWindow: 1_000_000 },
      thinkingLevel: "high",
      sessionManager: {
        getCwd: () => "/workspace/한글-project",
      },
      getContextUsage: () => ({ percent: 11, contextWindow: 1_000_000 }),
      ui: { theme: promptTheme() },
    };
    const data = { getGitBranch: () => "main", getExtensionStatuses: () => statuses };
    const render = createStatusLine(ctx as never, data as never, () => gitStatus);
    const line = (width = 300) => stripTerminalSequences(render(width));
    return { statuses, ctx, data, render, line };
  }

  test("preserves every status and places usage immediately before the model", () => {
    const { statuses, line } = fixture();
    expect(line()).toStartWith("╭\uE0B6 /workspace/한글-project › main \uE0B4");
    expect(line()).toContain("11%");
    expect(line()).not.toContain("/bg");
    expect(line()).not.toContain("tokens");
    expect(line()).not.toContain("↓");
    expect(line()).toEndWith("---------- 9% (⏳ 3d 14h) │ \uE0B6 test-model \uE0B4 · high");
    statuses.set("codex-fast-mode", "fast");
    statuses.set("custom", "other extension");
    expect(line()).toEndWith("test-model \uE0B4 · high · fast");
    expect(line()).toContain("other extension");
    statuses.set("codex-usage", "weekly unavailable (/codex-usage)");
    expect(line()).toContain("weekly unavailable");
    statuses.clear();
    expect(line()).not.toContain("⏳");
    expect(line(80)).toContain("test-model");
    expect(line(80)).toContain("11%");
  });

  test("puts Git counts after the branch inside its segment and preserves width bounds", () => {
    const { line, render, data } = fixture("*12 +4 ?13");
    expect(line()).toContain(" › main *12 +4 ?13 \uE0B4");
    expect(line().match(/\*12/g)).toHaveLength(1);
    for (let width = 0; width <= 320; width++) expect(visibleWidth(render(width))).toBeLessThanOrEqual(width);
    data.getGitBranch = () => "";
    expect(line()).not.toContain("*12");
    expect(fixture().line()).toContain(" › main \uE0B4");
  });

  test("shows fast only when enabled and follows model changes", () => {
    const { statuses, ctx, line } = fixture();
    statuses.delete("codex-fast-mode");
    expect(line()).not.toContain("fast");
    statuses.set("codex-fast-mode", "off");
    expect(line()).toEndWith("test-model \uE0B4 · high");
    statuses.set("codex-fast-mode", "fast");
    expect(line()).toEndWith("test-model \uE0B4 · high · fast");
    ctx.model = { ...ctx.model, id: "another-model", provider: "anthropic" };
    expect(line()).toContain("another-model");
    expect(line()).not.toContain("fast");
    ctx.model.provider = "openai";
    expect(line()).toEndWith("another-model \uE0B4 · high · fast");
    statuses.set("codex-fast-mode", "off");
    expect(line()).toEndWith("another-model \uE0B4 · high");
  });

  test.each([
    ["---------- 0% (⏳ unknown)", ""],
    ["#--------- 10% (⏳ unknown)", "#"],
    ["#######--- 79% (⏳ 3d 14h)", "#######"],
    ["########## 100% (⏳ unknown)", "##########"],
    ["weekly unavailable (/codex-usage)", ""],
  ])("colors only the filled quota cells green: %s", (usage, filled) => {
    const { statuses, ctx, render, line } = fixture();
    statuses.set("codex-usage", usage);
    expect(line()).toContain(usage);
    if (filled) {
      expect(render(300)).toContain(ctx.ui.theme.fg("success", filled) + usage.slice(filled.length));
    } else {
      expect(render(300)).not.toContain(ctx.ui.theme.getFgAnsi("success"));
    }
  });

  test("follows thinking levels and themes without terminal ANSI palette colors", () => {
    const { statuses, ctx, render } = fixture();
    statuses.set("codex-fast-mode", "fast");
    for (const name of ["one-half-light", "catppuccin-mocha"]) {
      ctx.ui.theme = promptTheme(name);
      for (const level of ["off", "minimal", "low", "medium", "high", "xhigh", "max"] as const) {
        ctx.thinkingLevel = level;
        expect(render(300)).toContain(ctx.ui.theme.getThinkingBorderColor(level)(level));
      }
      expect(render(300)).toContain(ctx.ui.theme.style("fast", { fg: "syntaxKeyword", bold: true }));
      expect(render(300)).not.toMatch(/\x1b\[(?:1;)?3[0-7]m/);
    }
  });

  test("uses distinct theme-derived badges and semantic Git colors", () => {
    const { ctx, render } = fixture("*12 +4 ?13");
    for (const name of ["one-half-light", "catppuccin-mocha"]) {
      ctx.ui.theme = promptTheme(name);
      const theme = ctx.ui.theme, colors = theme.colors;
      const ink = (color: typeof colors.accent) => mixColors(color, colors.text, 0.6, "srgb");
      const pathBg = mixColors(colors.userMessageBg, colors.accent, 0.12, "srgb");
      const modelBg = mixColors(colors.userMessageBg, colors.customMessageLabel, 0.12, "srgb");
      const output = render(300);
      expect(pathBg).not.toEqual(modelBg);
      const luminance = (color: typeof colors.accent) => {
        const { r, g, b } = colorToRgb(color);
        const [red, green, blue] = [r, g, b].map(channel => channel / 255)
          .map(channel => channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4);
        return red * 0.2126 + green * 0.7152 + blue * 0.0722;
      };
      for (const [fg, bg] of [
        ...[colors.accent, colors.customMessageLabel, colors.warning, colors.success, colors.error].map(color => [ink(color), pathBg]),
        [ink(colors.customMessageLabel), modelBg],
      ]) {
        const [a, b] = [luminance(fg), luminance(bg)].sort((a, b) => b - a);
        expect((a + 0.05) / (b + 0.05)).toBeGreaterThanOrEqual(4.5);
      }
      for (const bg of [pathBg, modelBg]) {
        expect(output).toContain(theme.style("\uE0B6", { fg: bg }));
        expect(output).toContain(theme.style("\uE0B4", { fg: bg }));
      }
      expect(output).toContain(theme.style("/workspace/한글-project", { fg: ink(colors.accent), bold: true }));
      expect(output).toContain(theme.style("test-model", { fg: ink(colors.customMessageLabel), bold: true }));
      for (const [text, color] of [["main", colors.customMessageLabel], ["*12", colors.warning], ["+4", colors.success], ["?13", colors.accent]] as const) {
        expect(output).toContain(theme.style(text, { fg: ink(color) }));
      }
    }
  });

  test("keeps the badge background continuous when a long path is shortened", () => {
    const { ctx, render } = fixture("*2 +4 ?1");
    ctx.sessionManager.getCwd = () => "/workspace/" + "long-directory/".repeat(8);
    const output = render(140);
    const badge = output.slice(output.indexOf("\uE0B6"), output.indexOf("\uE0B4"));
    expect(stripTerminalSequences(badge)).toContain("…");
    expect(stripTerminalSequences(badge)).toContain(" › main *2 +4 ?1 ");
    // A full reset emitted by truncation would clear the enclosing badge background.
    expect(badge).not.toContain("\x1b[0m");
  });

  test.each([[0, "accent"], [69, "accent"], [70, "warning"], [89, "warning"], [90, "error"], [100, "error"]] as const)(
    "uses the theme context threshold at %s percent", (percent, token) => {
      const { ctx, render } = fixture();
      ctx.getContextUsage = () => ({ percent, contextWindow: 1_000_000 });
      expect(render(300)).toContain(ctx.ui.theme.fg(token, `${percent}%`));
    },
  );

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
      expect(line(width)).toMatch(/[━─%M] │ (?:---------- 9%|\uE0B6 test-model|test-model)/);
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

  test("bounds real ANSI and wide text at every width", () => {
    const { statuses, render } = fixture();
    statuses.set("codex-usage", "#######--- 79% (⏳ 3d 14h)");
    for (let width = 0; width <= 320; width++) {
      expect(visibleWidth(render(width))).toBeLessThanOrEqual(width);
      expect(render(width)).not.toContain(CURSOR_MARKER);
    }
  });

  test("keeps scroll hints and handles missing context, long branches, and theme changes", () => {
    const { ctx, data, render } = fixture();
    expect(render(80, 7)).toContain("↑ 7 more");
    ctx.getContextUsage = () => undefined as never;
    data.getGitBranch = () => "한글".repeat(100);
    for (const width of [4, 20, 60, 100, 200]) expect(visibleWidth(render(width))).toBeLessThanOrEqual(width);
    expect(render(80)).toContain("?%");
    const light = render(80);
    ctx.ui.theme = promptTheme("catppuccin-mocha");
    expect(render(80)).not.toBe(light);
    expect(stripTerminalSequences(render(80))).toBe(stripTerminalSequences(light));
  });
});
