import { beforeEach, describe, expect, mock, test } from "bun:test";
import fastMode from "../files/extensions/codex-fast-mode";

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
    expect(setStatus).toHaveBeenLastCalledWith("codex-fast-mode", undefined);
    await command("on", tuiCtx);
    expect(setStatus).toHaveBeenLastCalledWith("codex-fast-mode", "fast");
    handlers.get("session_start")!({}, {
      ...tuiCtx,
      sessionManager: { getBranch: () => [{ type: "custom", customType: "codex-fast-mode", data: { enabled: true } }] },
    });
    expect(setStatus).toHaveBeenLastCalledWith("codex-fast-mode", "fast");
  });

  test("hides the label for other providers and children without changing the saved toggle", async () => {
    const setStatus = mock(() => {});
    const tuiCtx = { ...ctx, mode: "tui", model: { provider: "openai" }, ui: { ...ctx.ui, setStatus } };
    await command("on", tuiCtx);
    tuiCtx.model.provider = "anthropic";
    handlers.get("model_select")!({}, tuiCtx);
    expect(setStatus).toHaveBeenLastCalledWith("codex-fast-mode", undefined);
    tuiCtx.model.provider = "openai";
    handlers.get("model_select")!({}, tuiCtx);
    expect(setStatus).toHaveBeenLastCalledWith("codex-fast-mode", "fast");
    const previous = process.env.PI_SUBAGENT_CHILD;
    try {
      process.env.PI_SUBAGENT_CHILD = "1";
      handlers.get("model_select")!({}, tuiCtx);
      expect(setStatus).toHaveBeenLastCalledWith("codex-fast-mode", undefined);
    } finally {
      if (previous === undefined) delete process.env.PI_SUBAGENT_CHILD;
      else process.env.PI_SUBAGENT_CHILD = previous;
    }
    expect(appendEntry).toHaveBeenCalledTimes(1);
    expect(appendEntry).toHaveBeenCalledWith("codex-fast-mode", { enabled: true });
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
