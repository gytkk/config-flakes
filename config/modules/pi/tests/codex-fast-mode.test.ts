import { beforeEach, describe, expect, mock, test } from "bun:test";

mock.module("@earendil-works/pi-tui", () => ({
  truncateToWidth: (text: string, width: number) => text.slice(0, width),
  visibleWidth: (text: string) => text.length,
}));

const { default: fastMode, createTokenTotalsReader } = await import(
  "../files/extensions/codex-fast-mode"
);

function context() {
  let entries: any[] = [];
  let leaf = "root";
  let sessionId = "session-a";
  const getEntries = mock(() => entries);
  const ctx = {
    sessionManager: {
      getEntries,
      getLeafId: () => leaf,
      getSessionId: () => sessionId,
    },
  };
  return {
    ctx: ctx as never,
    getEntries,
    setEntries: (next: any[]) => {
      entries = next;
      leaf = `entry-${next.length}`;
    },
    setLeaf: (next: string) => { leaf = next; },
    setSessionId: (next: string) => { sessionId = next; },
  };
}

const usage = (input: number, output: number) => ({ input, output });

describe("footer token totals", () => {
  test("includes assistant, tool, summary, compaction, and standalone usage", () => {
    const fixture = context();
    fixture.setEntries([
      { type: "message", message: { role: "assistant", usage: usage(10, 1) } },
      { type: "message", message: { role: "toolResult", usage: usage(20, 2) } },
      { type: "message", message: { role: "toolResult" } },
      { type: "message", message: { role: "user" } },
      { type: "branch_summary", usage: usage(30, 3) },
      { type: "compaction", usage: usage(40, 4) },
      { type: "usage", kind: "cache_warm", usage: usage(50, 5) },
    ]);
    expect(createTokenTotalsReader()(fixture.ctx)).toEqual({ input: 150, output: 15 });
  });

  test("does not rescan unchanged sessions and invalidates on append or branch change", () => {
    const fixture = context();
    const readTotals = createTokenTotalsReader();
    expect(readTotals(fixture.ctx)).toEqual({ input: 0, output: 0 });
    readTotals(fixture.ctx);
    expect(fixture.getEntries).toHaveBeenCalledTimes(1);
    fixture.setEntries([{ type: "usage", usage: usage(5, 1) }]);
    expect(readTotals(fixture.ctx)).toEqual({ input: 5, output: 1 });
    expect(fixture.getEntries).toHaveBeenCalledTimes(2);
    fixture.setLeaf("branch-b");
    readTotals(fixture.ctx);
    expect(fixture.getEntries).toHaveBeenCalledTimes(3);
    fixture.setSessionId("session-b");
    readTotals(fixture.ctx);
    expect(fixture.getEntries).toHaveBeenCalledTimes(4);
    const other = context();
    readTotals(other.ctx);
    expect(other.getEntries).toHaveBeenCalledTimes(1);
  });
});

describe("Codex usage footer", () => {
  test("places usage immediately before the model on one bounded line", () => {
    const handlers = new Map<string, Function>();
    const statuses = new Map([["codex-usage", "---------- weekly 9% left | reset 2026-11-05 03:00 GMT+9"]]);
    let footer: { render: (width: number) => string[] };
    fastMode({
      on: (event: string, handler: Function) => handlers.set(event, handler),
      registerCommand: () => {},
    } as never);
    handlers.get("session_start")!({}, {
      mode: "tui",
      model: { id: "test-model", provider: "openai" },
      sessionManager: {
        getBranch: () => [], getEntries: () => [], getCwd: () => "/workspace",
        getSessionId: () => "test", getLeafId: () => null,
      },
      getContextUsage: () => undefined,
      ui: {
        setFooter: (factory: Function) => {
          footer = factory({ requestRender: () => {} }, {}, {
            onBranchChange: () => () => {}, getGitBranch: () => "main",
            getExtensionStatuses: () => statuses,
          });
        },
      },
    });
    for (const width of [0, 1, 2, 10, 40, 80, 120, 200]) {
      const lines = footer!.render(width);
      expect(lines).toHaveLength(1);
      expect(lines[0].length).toBeLessThanOrEqual(width);
    }
    const plainLine = footer!.render(200)[0].replace(/\x1b\[[0-9;]*m/g, "");
    expect(plainLine).toContain("---------- weekly 9% left | reset 2026-11-05 03:00 GMT+9 | test-model");
    expect(footer!.render(80)[0]).toContain("test-model");
    statuses.set("codex-usage", "weekly unavailable (/codex-usage)");
    expect(footer!.render(200)[0]).toContain("weekly unavailable");
    statuses.clear();
    const withoutUsage = footer!.render(80);
    expect(withoutUsage).toHaveLength(1);
    expect(withoutUsage[0]).not.toContain("Codex");
    expect(withoutUsage[0]).toContain("test-model");
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
