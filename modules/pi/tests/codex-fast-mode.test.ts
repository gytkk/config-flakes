import { beforeEach, describe, expect, mock, test } from "bun:test";

import { CURSOR_MARKER, visibleWidth } from "@earendil-works/pi-tui";
import fastMode, { createStatusLine } from "../files/extensions/codex-fast-mode";

describe("prompt statusline", () => {
  function fixture() {
    const statuses = new Map([["codex-usage", "---------- weekly 9% left | reset 2026-11-05 03:00"]]);
    const commands = ["bg"];
    const ctx = {
      model: { id: "test-model", provider: "openai", reasoning: true },
      thinkingLevel: "high",
      sessionManager: {
        getCwd: () => "/workspace/한글-project",
      },
      getContextUsage: () => ({ percent: 11 }),
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
    const render = createStatusLine({ getCommands: () => commands.map((name) => ({ name })) } as never, ctx as never, data as never);
    const line = (width = 300) => render(width).replace(/\x1b\[[0-9;]*m/g, "");
    return { statuses, commands, ctx, data, render, line };
  }

  test("preserves every status and places usage immediately before the model", () => {
    const { statuses, line } = fixture();
    expect(line()).toStartWith("╭\uE0B6 /workspace/한글-project › main \uE0B0");
    expect(line()).toContain("11%");
    expect(line()).toContain("/bg (0)");
    expect(line()).not.toContain("tokens");
    expect(line()).not.toContain("↓");
    expect(line()).toContain("---------- weekly 9% left | reset 2026-11-05 03:00 │ test-model · high · fast(off)");
    statuses.set("codex-fast-mode", "on");
    statuses.set("custom", "other extension");
    expect(line()).toContain("fast(on)");
    expect(line()).toContain("other extension");
    statuses.set("codex-usage", "weekly unavailable (/codex-usage)");
    expect(line()).toContain("weekly unavailable");
    statuses.clear();
    expect(line()).not.toContain("weekly");
    expect(line(80)).toContain("test-model");
    expect(line(80)).toContain("11%");
  });

  test("bounds real ANSI and wide text at every width, including selected tasks", () => {
    const { statuses, render } = fixture();
    for (const selected of [false, true]) {
      if (selected) statuses.set("bg-selection", "selected");
      for (let width = 0; width <= 320; width++) {
        expect(visibleWidth(render(width))).toBeLessThanOrEqual(width);
        if (selected && width > 0) expect(render(width)).toContain(CURSOR_MARKER);
        if (!selected) expect(render(width)).not.toContain(CURSOR_MARKER);
      }
    }
  });

  test("updates tasks without duplicating private statuses", () => {
    const { statuses, commands, line, render } = fixture();
    statuses.set("bg", "bg: 3");
    statuses.set("bg-selection", "selected");
    expect(line()).toContain(`${CURSOR_MARKER}* /bg (3)`);
    expect(line()).not.toContain("bg: 3");
    expect(line()).not.toContain("selected");
    expect(render(300)).toContain("\x1b[1;35m* /bg (3)\x1b[0m");
    statuses.delete("bg");
    expect(line()).toContain("* /bg (0)");
    statuses.delete("bg-selection");
    expect(line()).not.toContain(CURSOR_MARKER);
    statuses.set("bg", "unexpected status");
    expect(line()).toContain("/bg (?)");
    commands.length = 0;
    expect(line()).not.toContain("/bg");
    expect(line()).not.toContain("unexpected status");
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

describe("OpenAI fast mode", () => {
  let handlers: Map<string, Function>;
  let command: Function;
  let appendEntry: ReturnType<typeof mock>;
  const ctx = {
    model: { provider: "openai" },
    sessionManager: { getBranch: () => [] },
    ui: { notify: mock(() => {}) },
  };

  beforeEach(() => {
    handlers = new Map();
    appendEntry = mock(() => {});
    fastMode({
      on: (event: string, handler: Function) => handlers.set(event, handler),
      registerCommand: (_name: string, definition: { handler: Function }) => {
        command = definition.handler;
      },
      appendEntry,
    } as never);
  });

  test("publishes restored and toggled state without installing another footer", async () => {
    const setStatus = mock(() => {});
    const tuiCtx = { ...ctx, mode: "tui", ui: { ...ctx.ui, setStatus } };
    handlers.get("session_start")!({}, tuiCtx);
    expect(setStatus).toHaveBeenLastCalledWith("codex-fast-mode", "off");
    await command("on", tuiCtx);
    expect(setStatus).toHaveBeenLastCalledWith("codex-fast-mode", "on");
    handlers.get("session_start")!({}, {
      ...tuiCtx,
      sessionManager: { getBranch: () => [{ type: "custom", customType: "codex-fast-mode", data: { enabled: true } }] },
    });
    expect(setStatus).toHaveBeenLastCalledWith("codex-fast-mode", "on");
  });

  test("starts disabled and resets to disabled for a new session", async () => {
    const event = { payload: { model: "test" } };
    expect(handlers.get("before_provider_request")!(event, ctx)).toBeUndefined();
    await command("on", ctx);
    handlers.get("session_start")!({}, ctx);
    expect(handlers.get("before_provider_request")!(event, ctx)).toBeUndefined();
    expect(appendEntry).toHaveBeenCalledTimes(1);
  });

  test.each([true, false])("restores saved fast mode state: %s", (enabled) => {
    handlers.get("session_start")!({}, {
      ...ctx,
      sessionManager: {
        getBranch: () => [{
          type: "custom",
          customType: "codex-fast-mode",
          data: { enabled },
        }],
      },
    });
    const result = handlers.get("before_provider_request")!({ payload: {} }, ctx);
    expect(result).toEqual(enabled ? { service_tier: "priority" } : undefined);
  });

  test("toggles on from the default and explicitly turns off", async () => {
    await command("", ctx);
    expect(appendEntry).toHaveBeenLastCalledWith("codex-fast-mode", { enabled: true });
    expect(handlers.get("before_provider_request")!({ payload: {} }, ctx))
      .toEqual({ service_tier: "priority" });
    await command("off", ctx);
    expect(appendEntry).toHaveBeenLastCalledWith("codex-fast-mode", { enabled: false });
    expect(handlers.get("before_provider_request")!({ payload: {} }, ctx)).toBeUndefined();
  });

  test("applies priority to OpenAI without mutating the original payload", async () => {
    await command("on", ctx);
    const payload = { model: "test" };
    expect(handlers.get("before_provider_request")!({ payload }, { model: { provider: "openai" } }))
      .toEqual({ model: "test", service_tier: "priority" });
    expect(payload).toEqual({ model: "test" });
    expect(handlers.get("before_provider_request")!({ payload }, { model: { provider: "anthropic" } }))
      .toBeUndefined();
  });

  test("does not enable priority for subagent children", async () => {
    await command("on", ctx);
    const previous = process.env.PI_SUBAGENT_CHILD;
    process.env.PI_SUBAGENT_CHILD = "1";
    try {
      expect(handlers.get("before_provider_request")!({ payload: {} }, { model: { provider: "openai" } }))
        .toBeUndefined();
    } finally {
      if (previous === undefined) delete process.env.PI_SUBAGENT_CHILD;
      else process.env.PI_SUBAGENT_CHILD = previous;
    }
  });
});
