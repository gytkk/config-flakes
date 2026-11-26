import { afterEach, beforeEach, describe, expect, mock, setSystemTime, spyOn, test } from "bun:test";
import subscriptionUsage, { formatWeeklyUsage, parseWeeklyUsage, usageProviderFor } from "../files/extensions/subscription-usage";

const originalTimeZone = process.env.TZ;
beforeEach(() => { process.env.TZ = "UTC"; });

const NOW = Date.parse("2026-11-02T03:00:00Z");
const RESET = "2026-11-04T18:00:00Z";
function payload(secondary: unknown = { usedPercent: 91, windowMinutes: 10080, resetsAt: RESET }, provider = "codex") {
  return JSON.stringify([{
    provider, source: provider === "codex" ? "oauth" : "claude",
    usage: { secondary, updatedAt: new Date(NOW).toISOString() },
  }]);
}
const success = () => ({ stdout: payload(), stderr: "", code: 0, killed: false });
const cleanups: (() => void)[] = [];

const CODEX_ARGS = ["usage", "--provider", "codex", "--source", "oauth", "--json", "--no-credits"];
const CLAUDE_ARGS = ["usage", "--provider", "claude", "--source", "cli", "--json", "--no-credits"];

function fixture(mode = "tui", exec = mock(async () => success()), modelProvider = "openai") {
  const handlers = new Map<string, Function>();
  const commands = new Map<string, { handler: Function }>();
  const statuses = new Map<string, string>();
  const ctx = {
    mode,
    model: { provider: modelProvider, id: "test-model" } as { provider: string; id: string } | undefined,
    ui: {
      setStatus: mock((key: string, value?: string) => {
        if (value === undefined) statuses.delete(key);
        else statuses.set(key, value);
      }),
      notify: mock(() => {}),
    },
  };
  subscriptionUsage({
    on: (name: string, handler: Function) => handlers.set(name, handler),
    registerCommand: (name: string, command: { handler: Function }) => commands.set(name, command),
    exec,
  } as never);
  const stop = () => handlers.get("session_shutdown")?.({}, ctx);
  cleanups.push(stop);
  return {
    handlers, commands, ctx, exec, statuses, stop,
    start: () => handlers.get("session_start")?.({}, ctx),
    select: (provider: string) => {
      ctx.model = { provider, id: "test-model" };
      return handlers.get("model_select")?.({ type: "model_select", model: ctx.model, source: "set" }, ctx);
    },
    command: (args = "") => commands.get("usage")!.handler(args, ctx),
  };
}

afterEach(() => {
  cleanups.splice(0).forEach((stop) => stop());
  mock.restore();
  setSystemTime();
  if (originalTimeZone === undefined) delete process.env.TZ;
  else process.env.TZ = originalTimeZone;
});

describe("CodexBar weekly data", () => {
  test("maps model providers to subscription usage sources", () => {
    expect(usageProviderFor("openai")).toBe("codex");
    expect(usageProviderFor("openai-codex")).toBe("codex");
    expect(usageProviderFor("claude-sdk")).toBe("claude");
    expect(usageProviderFor("anthropic")).toBe("claude");
    expect(usageProviderFor("google")).toBeUndefined();
    expect(usageProviderFor(undefined)).toBeUndefined();
  });

  test("reads the Claude weekly lane only for Claude output", () => {
    expect(parseWeeklyUsage(payload({ usedPercent: 73, windowMinutes: 10080, resetsAt: RESET }, "claude"), "claude"))
      .toEqual({ remainingPercent: 27, resetsAt: Date.parse(RESET), updatedAt: NOW });
    expect(() => parseWeeklyUsage(payload(), "claude")).toThrow("Claude account");
    expect(() => parseWeeklyUsage(JSON.stringify([{ provider: "claude", error: { message: "private" } }]), "claude"))
      .toThrow("Claude Code login");
  });

  test("shows only known Fable quota next to Claude's weekly remaining quota", () => {
    const format = (window: unknown, usageKnown = true, provider: "claude" | "codex" = "claude") => {
      const entries = JSON.parse(payload({ usedPercent: 70, resetsAt: RESET }, provider));
      entries[0].usage.extraRateWindows = [
        { id: "claude-weekly-scoped-opus", window: { usedPercent: 5 } },
        { id: "claude-weekly-scoped-fable", window, usageKnown },
      ];
      return formatWeeklyUsage(parseWeeklyUsage(JSON.stringify(entries), provider), NOW);
    };
    expect(format({ usedPercent: 78 })).toBe("###------- 30% | Fable 22%");
    expect(format({ usedPercent: 100 })).toBe("###------- 30% | Fable 0%");
    const fallback = "###------- 30% (⏳ 2d 15h)";
    expect(format({ usedPercent: 0 }, false)).toBe(fallback);
    expect(format({ usedPercent: 0, isSyntheticPlaceholder: true })).toBe(fallback);
    expect(format({ usedPercent: 101 })).toBe(fallback);
    expect(format({ usedPercent: 78 }, true, "codex")).toBe(fallback);
  });

  test("reads the weekly lane, preserves zero, and ignores account fields", () => {
    const data = parseWeeklyUsage(payload());
    expect(data).toEqual({ remainingPercent: 9, resetsAt: Date.parse(RESET), updatedAt: NOW });
    expect(parseWeeklyUsage(payload({ usedPercent: 100 })).remainingPercent).toBe(0);
    expect(parseWeeklyUsage(payload({ usedPercent: 0 })).remainingPercent).toBe(100);
  });

  test("rejects invalid, ambiguous, failed, and missing weekly data", () => {
    for (const raw of [
      "not json", "null", "{}", "[]",
      JSON.stringify([{ provider: "claude" }]),
      JSON.stringify([{ provider: "codex", error: { message: "private data" } }]),
      JSON.stringify([...JSON.parse(payload()), ...JSON.parse(payload())]),
      payload(null), payload({ usedPercent: "91" }), payload({ usedPercent: -1 }),
      payload({ usedPercent: 101 }), payload({ usedPercent: 10, windowMinutes: 300 }),
      payload({ usedPercent: 10, resetsAt: "invalid" }),
      payload().replace(new Date(NOW).toISOString(), "invalid"),
    ]) {
      expect(() => parseWeeklyUsage(raw)).toThrow();
    }
  });

  test.each([
    [0, "----------"],
    [9, "----------"],
    [10, "#---------"],
    [33, "###-------"],
    [79, "#######---"],
    [99, "#########-"],
    [100, "##########"],
  ])("renders a ten-cell remaining-quota bar at %s percent", (remainingPercent, bar) => {
    const usage = { ...parseWeeklyUsage(payload()), remainingPercent };
    const text = formatWeeklyUsage(usage, NOW);
    expect(text).toBe(`${bar} ${remainingPercent}% (⏳ 2d 15h)`);
    expect(text).not.toContain("Codex");
  });

  test("formats reset boundaries without inventing a reset or new quota", () => {
    const data = parseWeeklyUsage(payload());
    expect(formatWeeklyUsage(data, NOW)).toBe("---------- 9% (⏳ 2d 15h)");
    for (const [milliseconds, expected] of [
      [86 * 3_600_000 + 59 * 60_000, "3d 14h"],
      [24 * 3_600_000, "1d 0h"],
      [24 * 3_600_000 - 1, "0d 23h"],
      [3_600_000, "0d 1h"],
      [3_600_000 - 1, "0d 0h"],
      [1, "0d 0h"],
    ] as const) {
      expect(formatWeeklyUsage({ ...data, resetsAt: NOW + milliseconds }, NOW))
        .toBe(`---------- 9% (⏳ ${expected})`);
    }
    expect(formatWeeklyUsage({ ...data, resetsAt: NOW - 1 }, NOW)).toContain("awaiting fresh usage");
    expect(formatWeeklyUsage({ ...data, resetsAt: NOW }, NOW)).toContain("awaiting fresh usage");
    expect(formatWeeklyUsage({ ...data, resetsAt: undefined }, NOW)).toBe("---------- 9% (⏳ unknown)");
    expect(formatWeeklyUsage(data, NOW + 600_001)).toContain("stale data");
    expect(formatWeeklyUsage(data, NOW - 60_001)).toContain("stale data");
  });
});

test.each([
  ["Asia/Seoul", "2026-11-30T15:00:00Z", "2026-12-01 00:00"],
  ["Asia/Seoul", "2026-12-31T15:05:00Z", "2027-01-01 00:05"],
  ["America/Los_Angeles", "2026-11-04T18:00:00Z", "2026-11-04 10:00"],
  ["America/Los_Angeles", "2026-07-04T18:00:00Z", "2026-07-04 11:00"],
  ["Asia/Kolkata", "2026-11-04T18:00:00Z", "2026-11-04 23:30"],
])("keeps the countdown timezone-independent and detailed reset local in %s for %s", async (timeZone, timestamp, expected) => {
  process.env.TZ = timeZone;
  const resetsAt = Date.parse(timestamp);
  const now = resetsAt - 86 * 3_600_000;
  const usage = { remainingPercent: 33, resetsAt, updatedAt: now };
  expect(formatWeeklyUsage(usage, now)).toBe("###------- 33% (⏳ 3d 14h)");
  setSystemTime(now);
  const stdout = payload({ usedPercent: 67, resetsAt: timestamp })
    .replace(new Date(NOW).toISOString(), new Date(now).toISOString());
  const f = fixture("tui", mock(async () => ({ ...success(), stdout })));
  await f.command();
  expect(f.ctx.ui.notify.mock.calls[0][0]).toContain(`Reset: ${expected}`);
});

describe("Codex usage lifecycle", () => {
  test("fetches through CodexBar and shares the startup request with a command", async () => {
    setSystemTime(NOW);
    const f = fixture();
    f.start();
    await f.command();
    expect(f.exec).toHaveBeenCalledTimes(1);
    expect(f.exec.mock.calls[0]).toEqual([
      "codexbar", CODEX_ARGS,
      { timeout: 30_000, signal: expect.any(AbortSignal) },
    ]);
    expect(f.statuses.get("subscription-usage")).toBe("---------- 9% (⏳ 2d 15h)");
    expect(f.ctx.ui.notify.mock.calls[0][0]).toStartWith("Codex ---------- 9%");
    expect(f.ctx.ui.notify.mock.calls[0][0]).toContain("Codex CLI account, not Pi login");
    expect(f.ctx.ui.notify.mock.calls[0][0]).toContain("Reset: 2026-11-04 18:00");
    await f.command();
    expect(f.exec).toHaveBeenCalledTimes(2);
  });

  test("polls usage every minute and clears the timer on shutdown", async () => {
    setSystemTime(NOW);
    let tick: () => void = () => {};
    const timer = { unref: mock(() => {}) };
    const interval = spyOn(globalThis, "setInterval").mockImplementation(((callback: () => void) => {
      tick = callback;
      return timer;
    }) as never);
    const clear = spyOn(globalThis, "clearInterval").mockImplementation(() => {});
    const f = fixture();
    f.start();
    await f.command();
    expect(interval.mock.calls[0][1]).toBe(60_000);
    expect(timer.unref).toHaveBeenCalledTimes(1);
    setSystemTime(NOW + 60_000);
    tick();
    await f.command();
    expect(f.exec).toHaveBeenCalledTimes(2);
    expect(f.statuses.get("subscription-usage")).toBe("---------- 9% (⏳ 2d 14h)");
    f.stop();
    expect(clear).toHaveBeenCalledWith(timer);
    expect(f.statuses.size).toBe(0);
  });

  test("does not poll outside the interactive parent", async () => {
    for (const mode of ["rpc", "json", "print"]) {
      const f = fixture(mode);
      f.start();
      expect(f.exec).not.toHaveBeenCalled();
      expect(f.statuses.size).toBe(0);
      f.stop();
    }
    const previous = process.env.PI_SUBAGENT_CHILD;
    try {
      process.env.PI_SUBAGENT_CHILD = "1";
      const f = fixture();
      f.start();
      expect(f.handlers.size).toBe(0);
      expect(f.commands.size).toBe(0);
      expect(f.exec).not.toHaveBeenCalled();
    } finally {
      if (previous === undefined) delete process.env.PI_SUBAGENT_CHILD;
      else process.env.PI_SUBAGENT_CHILD = previous;
    }
  });

  test("replaces old readings on failure and recovers on the next command", async () => {
    setSystemTime(NOW);
    const f = fixture();
    f.start();
    await f.command();
    f.exec.mockResolvedValueOnce({ stdout: "secret", stderr: "secret", code: 1, killed: false });
    await f.command();
    expect(f.statuses.get("subscription-usage")).toBe("weekly unavailable (/usage)");
    const calls = f.ctx.ui.notify.mock.calls;
    expect(calls.at(-1)).toEqual([expect.stringContaining("code 1"), "error"]);
    expect(JSON.stringify(calls)).not.toContain("secret");
    await f.command();
    expect(f.statuses.get("subscription-usage")).toContain("9% (⏳ ");
  });

  test("reports missing binary and timeouts without leaking CLI errors", async () => {
    const f = fixture("rpc");
    f.start();
    f.exec.mockRejectedValueOnce(new Error("secret credential"));
    await f.command();
    expect(f.ctx.ui.notify.mock.calls.at(-1)).toEqual([expect.stringContaining("PATH"), "error"]);
    f.exec.mockResolvedValueOnce({ ...success(), killed: true });
    await f.command();
    expect(f.ctx.ui.notify.mock.calls.at(-1)).toEqual([expect.stringContaining("timed out"), "error"]);
    expect(JSON.stringify(f.ctx.ui.notify.mock.calls)).not.toContain("secret");
  });

  test("cancels in-flight work and prevents stale results after session replacement", async () => {
    setSystemTime(NOW);
    let resolve!: (value: ReturnType<typeof success>) => void;
    const exec = mock(() => new Promise<ReturnType<typeof success>>((done) => { resolve = done; }));
    const f = fixture("tui", exec);
    f.start();
    const pending = f.command();
    const oldResolve = resolve;
    const signal = (exec.mock.calls[0] as any)[2].signal as AbortSignal;
    f.stop();
    expect(signal.aborted).toBe(true);
    f.start();
    resolve(success());
    await f.command();
    const updates = f.ctx.ui.setStatus.mock.calls.length;
    oldResolve({ ...success(), stdout: payload({ usedPercent: 100 }) });
    await pending;
    expect(f.ctx.ui.setStatus).toHaveBeenCalledTimes(updates);
    expect(f.statuses.get("subscription-usage")).toContain("9% (⏳ ");
    expect(f.ctx.ui.notify).toHaveBeenCalledTimes(1);
  });

  test("rejects command arguments without launching a process", async () => {
    const f = fixture("rpc");
    await f.command("unexpected");
    expect(f.exec).not.toHaveBeenCalled();
    expect(f.ctx.ui.notify).toHaveBeenCalledWith("Usage: /usage", "warning");
  });
});

describe("provider-aware usage", () => {
  const claudeSuccess = () => ({ ...success(), stdout: payload({ usedPercent: 73, windowMinutes: 10080, resetsAt: RESET }, "claude") });
  const byProvider = () => mock(async (_cmd: string, args: string[]) => args[2] === "claude" ? claudeSuccess() : success());

  test("fetches Claude usage through the CLI source with a longer timeout", async () => {
    setSystemTime(NOW);
    const f = fixture("tui", byProvider(), "claude-sdk");
    f.start();
    await f.command();
    expect(f.exec).toHaveBeenCalledTimes(1);
    expect(f.exec.mock.calls[0]).toEqual(["codexbar", CLAUDE_ARGS, { timeout: 60_000, signal: expect.any(AbortSignal) }]);
    expect(f.statuses.get("subscription-usage")).toBe("##-------- 27% (⏳ 2d 15h)");
    const message = f.ctx.ui.notify.mock.calls[0][0];
    expect(message).toStartWith("Claude ##-------- 27%");
    expect(message).toContain("Claude Code account");
  });

  test("switches the status with the selected model and reuses cached readings", async () => {
    setSystemTime(NOW);
    const f = fixture("tui", byProvider());
    f.start();
    await f.command();
    expect(f.statuses.get("subscription-usage")).toContain("9%");
    f.select("claude-sdk");
    expect(f.statuses.get("subscription-usage")).toBe("weekly loading...");
    await f.command();
    expect(f.statuses.get("subscription-usage")).toContain("27%");
    const calls = f.exec.mock.calls.length;
    f.select("openai");
    expect(f.statuses.get("subscription-usage")).toContain("9%");
    expect(f.exec).toHaveBeenCalledTimes(calls);
    f.select("google");
    expect(f.statuses.has("subscription-usage")).toBe(false);
  });

  test("polls Claude every five minutes and Codex every minute", async () => {
    setSystemTime(NOW);
    let tick: () => void = () => {};
    spyOn(globalThis, "setInterval").mockImplementation(((callback: () => void) => {
      tick = callback;
      return { unref() {} };
    }) as never);
    spyOn(globalThis, "clearInterval").mockImplementation(() => {});
    const f = fixture("tui", byProvider(), "claude-sdk");
    f.start();
    await Promise.resolve();
    expect(f.exec).toHaveBeenCalledTimes(1);
    for (let minute = 1; minute < 5; minute++) {
      setSystemTime(NOW + minute * 60_000);
      tick();
    }
    expect(f.exec).toHaveBeenCalledTimes(1);
    setSystemTime(NOW + 5 * 60_000);
    tick();
    expect(f.exec).toHaveBeenCalledTimes(2);
    f.select("openai");
    expect(f.exec).toHaveBeenCalledTimes(3);
    await new Promise((settled) => setTimeout(settled, 0));
    setSystemTime(NOW + 6 * 60_000);
    tick();
    expect(f.exec).toHaveBeenCalledTimes(4);
    expect(f.exec.mock.calls.map(([, args]) => args[2])).toEqual(["claude", "claude", "codex", "codex"]);
  });

  test("explains unsupported providers without launching a process", async () => {
    const f = fixture("tui", byProvider(), "google");
    f.start();
    expect(f.statuses.has("subscription-usage")).toBe(false);
    await f.command();
    expect(f.exec).not.toHaveBeenCalled();
    expect(f.ctx.ui.notify).toHaveBeenCalledWith(expect.stringContaining("provider google"), "warning");
  });
});
