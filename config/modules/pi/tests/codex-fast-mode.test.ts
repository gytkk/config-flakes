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

describe("OpenAI fast mode", () => {
  let handlers: Map<string, Function>;
  beforeEach(() => {
    handlers = new Map();
    fastMode({
      on: (event: string, handler: Function) => handlers.set(event, handler),
      registerCommand: () => {},
    } as never);
  });

  test("applies priority to OpenAI without mutating the original payload", () => {
    const payload = { model: "test" };
    expect(handlers.get("before_provider_request")!({ payload }, { model: { provider: "openai" } }))
      .toEqual({ model: "test", service_tier: "priority" });
    expect(payload).toEqual({ model: "test" });
    expect(handlers.get("before_provider_request")!({ payload }, { model: { provider: "anthropic" } }))
      .toBeUndefined();
  });

  test("does not enable priority for subagent children", () => {
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
