import { afterEach, beforeEach, describe, expect, mock, setSystemTime, spyOn, test } from "bun:test";
import codexUsage, { formatWeeklyUsage, parseWeeklyUsage } from "../files/extensions/codex-usage";

const originalTimeZone = process.env.TZ;
beforeEach(() => { process.env.TZ = "UTC"; });

const NOW = Date.parse("2026-11-02T03:00:00Z");
const RESET = "2026-11-04T18:00:00Z";
function payload(secondary: unknown = { usedPercent: 91, windowMinutes: 10080, resetsAt: RESET }) {
  return JSON.stringify([{
    provider: "codex", source: "oauth",
    usage: { secondary, updatedAt: new Date(NOW).toISOString() },
  }]);
}
const success = () => ({ stdout: payload(), stderr: "", code: 0, killed: false });
const cleanups: (() => void)[] = [];

function fixture(mode = "tui", exec = mock(async () => success())) {
  const handlers = new Map<string, Function>();
  const commands = new Map<string, { handler: Function }>();
  const statuses = new Map<string, string>();
  const ctx = {
    mode,
    ui: {
      setStatus: mock((key: string, value?: string) => {
        if (value === undefined) statuses.delete(key);
        else statuses.set(key, value);
      }),
      notify: mock(() => {}),
    },
  };
  codexUsage({
    on: (name: string, handler: Function) => handlers.set(name, handler),
    registerCommand: (name: string, command: { handler: Function }) => commands.set(name, command),
    exec,
  } as never);
  const stop = () => handlers.get("session_shutdown")?.({}, ctx);
  cleanups.push(stop);
  return {
    handlers, commands, ctx, exec, statuses, stop,
    start: () => handlers.get("session_start")?.({}, ctx),
    command: (args = "") => commands.get("codex-usage")!.handler(args, ctx),
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
    [99, "#########-"],
    [100, "##########"],
  ])("renders a ten-cell remaining-quota bar at %s percent", (remainingPercent, bar) => {
    const usage = { ...parseWeeklyUsage(payload()), remainingPercent };
    const text = formatWeeklyUsage(usage, NOW);
    expect(text).toStartWith(`${bar} weekly ${remainingPercent}% left | reset `);
    expect(text).not.toContain("Codex");
  });

  test("formats reset boundaries without inventing a reset or new quota", () => {
    const data = parseWeeklyUsage(payload());
    expect(formatWeeklyUsage(data, NOW)).toMatch(/^---------- weekly 9% left \| reset 2026-11-04 18:00 GMT(?:\+0)?$/);
    expect(formatWeeklyUsage({ ...data, resetsAt: NOW + 3_600_000 }, NOW)).toContain("2026-11-02 04:00 GMT");
    expect(formatWeeklyUsage({ ...data, resetsAt: NOW + 1 }, NOW)).toContain("2026-11-02 03:00 GMT");
    expect(formatWeeklyUsage({ ...data, resetsAt: NOW }, NOW)).toContain("awaiting fresh usage");
    expect(formatWeeklyUsage({ ...data, resetsAt: undefined }, NOW)).toContain("reset unknown");
    expect(formatWeeklyUsage(data, NOW + 600_001)).toContain("stale data");
    expect(formatWeeklyUsage(data, NOW - 60_001)).toContain("stale data");
  });
});

test.each([
  ["Asia/Seoul", "2026-11-30T15:00:00Z", "2026-12-01 00:00 GMT+9"],
  ["Asia/Seoul", "2026-12-31T15:05:00Z", "2027-01-01 00:05 GMT+9"],
  ["America/Los_Angeles", "2026-11-04T18:00:00Z", "2026-11-04 10:00 GMT-8"],
  ["America/Los_Angeles", "2026-07-04T18:00:00Z", "2026-07-04 11:00 GMT-7"],
  ["Asia/Kolkata", "2026-11-04T18:00:00Z", "2026-11-04 23:30 GMT+5:30"],
])("uses the host time zone %s for %s", (timeZone, timestamp, expected) => {
  process.env.TZ = timeZone;
  const resetsAt = Date.parse(timestamp);
  const now = resetsAt - 60_000;
  const usage = { remainingPercent: 33, resetsAt, updatedAt: now };
  expect(formatWeeklyUsage(usage, now)).toBe(`###------- weekly 33% left | reset ${expected}`);
});

describe("Codex usage lifecycle", () => {
  test("fetches through CodexBar and shares the startup request with a command", async () => {
    setSystemTime(NOW);
    const f = fixture();
    f.start();
    await f.command();
    expect(f.exec).toHaveBeenCalledTimes(1);
    expect(f.exec.mock.calls[0]).toEqual([
      "codexbar", ["usage", "--provider", "codex", "--source", "oauth", "--json", "--no-credits"],
      { timeout: 30_000, signal: expect.any(AbortSignal) },
    ]);
    expect(f.statuses.get("codex-usage")).toMatch(/^---------- weekly 9% left \| reset 2026-11-04 18:00 GMT(?:\+0)?$/);
    expect(f.ctx.ui.notify.mock.calls[0][0]).toContain("Codex CLI account, not Pi login");
    expect(f.ctx.ui.notify.mock.calls[0][0]).toContain("Reset: 2026-11-04 18:00 GMT");
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
    expect(f.statuses.get("codex-usage")).toContain("2026-11-04 18:00 GMT");
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
    expect(f.statuses.get("codex-usage")).toBe("weekly unavailable (/codex-usage)");
    const calls = f.ctx.ui.notify.mock.calls;
    expect(calls.at(-1)).toEqual([expect.stringContaining("code 1"), "error"]);
    expect(JSON.stringify(calls)).not.toContain("secret");
    await f.command();
    expect(f.statuses.get("codex-usage")).toContain("9% left");
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
    expect(f.statuses.get("codex-usage")).toContain("9% left");
    expect(f.ctx.ui.notify).toHaveBeenCalledTimes(1);
  });

  test("rejects command arguments without launching a process", async () => {
    const f = fixture("rpc");
    await f.command("unexpected");
    expect(f.exec).not.toHaveBeenCalled();
    expect(f.ctx.ui.notify).toHaveBeenCalledWith("Usage: /codex-usage", "warning");
  });
});
