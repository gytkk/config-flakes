import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import herdrSubagents, {
  foregroundChildren,
  sidebarTokens,
} from "../files/extensions/herdr-subagents";

type Handler = (event: any, context: any) => unknown;
type ExecResult = { code: number; killed: boolean; stderr: string };

const originalEnv = { ...process.env };
const waitForPublish = () => Bun.sleep(300);

function fakePi(
  run: (binary: string, args: string[]) => Promise<ExecResult> = async () => ({
    code: 0,
    killed: false,
    stderr: "",
  }),
) {
  const handlers = new Map<string, Handler[]>();
  const eventHandlers = new Map<string, Handler[]>();
  const execCalls: Array<{ binary: string; args: string[] }> = [];

  const pi = {
    on(event: string, handler: Handler) {
      handlers.set(event, [...(handlers.get(event) ?? []), handler]);
    },
    async exec(binary: string, args: string[]) {
      execCalls.push({ binary, args });
      return run(binary, args);
    },
    events: {
      on(event: string, handler: Handler) {
        eventHandlers.set(event, [...(eventHandlers.get(event) ?? []), handler]);
        return () => {
          eventHandlers.set(
            event,
            (eventHandlers.get(event) ?? []).filter((candidate) => candidate !== handler),
          );
        };
      },
    },
  };

  return {
    execCalls,
    eventHandlers,
    handlers,
    install() {
      herdrSubagents(pi as never);
    },
    async emit(event: string, payload: any = {}, context: any = undefined) {
      await Promise.all(
        (handlers.get(event) ?? []).map((handler) => handler(payload, context)),
      );
    },
    async emitEvent(event: string, payload: any = {}) {
      await Promise.all(
        (eventHandlers.get(event) ?? []).map((handler) => handler(payload, undefined)),
      );
    },
  };
}

function reportTokens(args: string[]): Record<string, string | null> {
  const tokens: Record<string, string | null> = {};
  for (let index = 0; index < args.length; index++) {
    if (args[index] === "--token") {
      const [key, ...value] = args[++index].split("=");
      tokens[key] = value.join("=");
    } else if (args[index] === "--clear-token") {
      tokens[args[++index]] = null;
    }
  }
  return tokens;
}

function running(index: number, overrides: Record<string, unknown> = {}) {
  return {
    index,
    agent: `worker-${index}`,
    status: "running",
    task: `task ${index}`,
    model: `model-${index}`,
    currentTool: "read",
    currentPath: `/tmp/file-${index}`,
    recentOutput: [],
    ...overrides,
  };
}

async function startTui(
  harness: ReturnType<typeof fakePi>,
  notifications: string[] = [],
  context: Record<string, unknown> = { model: { id: "parent-model" } },
) {
  await harness.emit("session_start", {}, {
    mode: "tui",
    ...context,
    ui: {
      notify(message: string) {
        notifications.push(message);
      },
    },
  });
}

async function update(
  harness: ReturnType<typeof fakePi>,
  toolCallId: string,
  progress: unknown,
) {
  await harness.emit("tool_execution_start", {
    toolName: "subagent",
    toolCallId,
  });
  await harness.emit("tool_execution_update", {
    toolName: "subagent",
    toolCallId,
    partialResult: { details: { progress } },
  });
  await waitForPublish();
}

beforeEach(() => {
  process.env.HERDR_ENV = "1";
  process.env.HERDR_PANE_ID = "pane-42";
  process.env.HERDR_BIN_PATH = "/opt/herdr";
  delete process.env.PI_SUBAGENT_CHILD;
});

afterEach(() => {
  for (const key of Object.keys(process.env)) {
    if (!(key in originalEnv)) delete process.env[key];
  }
  Object.assign(process.env, originalEnv);
});

describe("foreground child projection", () => {
  test("shows identity, model, tool, and path for active children", () => {
    expect(foregroundChildren({
      progress: [running(2, { agent: "reviewer", model: "sonnet" })],
    })).toEqual([{
      index: 2,
      agent: "reviewer",
      model: "sonnet",
      activity: "read /tmp/file-2",
    }]);
  });

  test("uses recent output and then task as the activity fallback", () => {
    expect(foregroundChildren({
      progress: [
        running(0, {
          currentTool: "",
          recentOutput: ["older", "\u001b[31mlatest\u001b[0m\nline"],
        }),
        running(1, { currentTool: "", recentOutput: [], task: "inspect tests" }),
        running(2, { status: "pending", task: "wait for a slot" }),
      ],
    })?.map((child) => child.activity)).toEqual([
      "latest line",
      "inspect tests",
      "pending: wait for a slot",
    ]);
  });

  test("ignores finished and malformed progress without throwing", () => {
    expect(foregroundChildren({ progress: [
      null,
      { index: -1, agent: "negative", status: "running" },
      { index: 0, agent: "", status: "running" },
      running(1, { status: "completed" }),
      running(2, { status: "failed" }),
      running(3),
    ] })).toEqual([{
      index: 3,
      agent: "worker-3",
      model: "model-3",
      activity: "read /tmp/file-3",
    }]);
    expect(foregroundChildren({ progress: "not-an-array" })).toBeUndefined();
    expect(foregroundChildren(null)).toBeUndefined();
  });
});

describe("sidebar token projection", () => {
  test("clears unused activity rows when overflow replaces the seventh child", () => {
    const seven = sidebarTokens(
      Array.from({ length: 7 }, (_, index) => ({
        index,
        agent: `worker-${index}`,
        model: "model",
        activity: `activity-${index}`,
      })),
    );
    const eight = sidebarTokens(
      Array.from({ length: 8 }, (_, index) => ({
        index,
        agent: `worker-${index}`,
        model: "model",
        activity: `activity-${index}`,
      })),
    );

    expect(seven.subagent_7_activity).toBe("|  activity-6");
    expect(eight.subagent_7).toBe("|- +2 more subagents");
    expect(eight.subagent_7_activity).toBeNull();
  });
});

describe("Herdr extension reporting", () => {
  test("runs only for a parent TUI inside a Herdr pane", async () => {
    const parent = fakePi();
    parent.install();
    await startTui(parent);

    expect(parent.execCalls).toHaveLength(1);
    expect(parent.execCalls[0].binary).toBe("/opt/herdr");
    expect(parent.execCalls[0].args.slice(0, 9)).toEqual([
      "pane", "report-metadata", "pane-42", "--source", "pi:herdr-subagents",
      "--agent", "pi",
      "--ttl-ms", "45000",
    ]);
    expect(reportTokens(parent.execCalls[0].args).pi_model).toBe("parent-model");

    const headless = fakePi();
    headless.install();
    await headless.emit("session_start", {}, { mode: "rpc", ui: { notify() {} } });
    expect(headless.execCalls).toHaveLength(0);

    process.env.PI_SUBAGENT_CHILD = "1";
    const child = fakePi();
    child.install();
    expect(child.handlers.size).toBe(0);
    expect(child.execCalls).toHaveLength(0);

    delete process.env.PI_SUBAGENT_CHILD;
    delete process.env.HERDR_ENV;
    const outsideHerdr = fakePi();
    outsideHerdr.install();
    expect(outsideHerdr.handlers.size).toBe(0);

    await parent.emit("session_shutdown");
  });

  test("reports the session model, follows model changes, and clears it on shutdown", async () => {
    const harness = fakePi();
    harness.install();
    await startTui(harness, [], { model: { id: "initial-model" } });

    expect(reportTokens(harness.execCalls.at(-1)!.args)).toMatchObject({
      pi_model: "initial-model",
      subagent_1: null,
    });

    await harness.emit("model_select", { model: { id: "event-model" } }, {
      model: { id: "selected-model" },
    });
    expect(reportTokens(harness.execCalls.at(-1)!.args).pi_model).toBe("selected-model");

    await harness.emit("session_shutdown");
    expect(reportTokens(harness.execCalls.at(-1)!.args).pi_model).toBeNull();
  });

  test("clears missing session and selected models", async () => {
    const harness = fakePi();
    harness.install();
    await startTui(harness, [], {});
    expect(reportTokens(harness.execCalls.at(-1)!.args).pi_model).toBeNull();

    await harness.emit("model_select", { model: { id: "event-model" } }, {
      model: undefined,
    });
    expect(reportTokens(harness.execCalls.at(-1)!.args).pi_model).toBeNull();

    await harness.emit("session_shutdown");
  });

  test("keeps concurrent tool calls separate and removes only the finished call", async () => {
    const harness = fakePi();
    harness.install();
    await startTui(harness);
    await update(harness, "call-a", [running(0, { agent: "alpha" })]);
    await update(harness, "call-b", [running(0, { agent: "beta" })]);

    expect(reportTokens(harness.execCalls.at(-1)!.args)).toMatchObject({
      subagent_1: "|- alpha / model-0",
      subagent_2: "|- beta / model-0",
    });

    await harness.emit("tool_execution_end", {
      toolName: "subagent",
      toolCallId: "call-a",
      isError: true,
    });
    await waitForPublish();
    expect(reportTokens(harness.execCalls.at(-1)!.args)).toMatchObject({
      subagent_1: "|- beta / model-0",
      subagent_2: null,
    });

    await harness.emit("session_shutdown");
  });

  test("tracks slash subagents independently and preserves them across parent agent_end", async () => {
    const harness = fakePi();
    harness.install();
    expect(harness.eventHandlers.size).toBe(0);
    await startTui(harness);

    await harness.emitEvent("subagent:slash:started", { requestId: "slash-1" });
    await harness.emitEvent("subagent:slash:update", {
      requestId: "slash-1",
      progress: [running(0, { agent: "slash-worker" })],
    });
    await update(harness, "tool-1", [running(0, { agent: "tool-worker" })]);

    expect(reportTokens(harness.execCalls.at(-1)!.args)).toMatchObject({
      subagent_1: "|- slash-worker / model-0",
      subagent_2: "|- tool-worker / model-0",
    });

    await harness.emit("agent_end");
    expect(reportTokens(harness.execCalls.at(-1)!.args)).toMatchObject({
      subagent_1: "|- slash-worker / model-0",
      subagent_2: null,
    });

    await harness.emitEvent("subagent:slash:response", {
      requestId: "slash-1",
      result: {},
      isError: true,
    });
    await waitForPublish();
    expect(reportTokens(harness.execCalls.at(-1)!.args).subagent_1).toBeNull();

    await harness.emit("session_shutdown");
    expect(
      [...harness.eventHandlers.values()].flat(),
    ).toHaveLength(0);
  });

  test("clears sidebar rows after success, abort, and shutdown", async () => {
    const harness = fakePi();
    harness.install();
    await startTui(harness);

    await update(harness, "finished", [running(0)]);
    await harness.emit("tool_execution_end", {
      toolName: "subagent",
      toolCallId: "finished",
      isError: false,
    });
    await waitForPublish();
    expect(reportTokens(harness.execCalls.at(-1)!.args).subagent_1).toBeNull();

    await update(harness, "aborted", [running(0)]);
    await harness.emit("agent_end");
    expect(reportTokens(harness.execCalls.at(-1)!.args).subagent_1).toBeNull();

    await update(harness, "shutdown", [running(0)]);
    await harness.emit("session_shutdown");
    expect(reportTokens(harness.execCalls.at(-1)!.args).subagent_1).toBeNull();
  });

  test("warns once for repeated failures and warns again after a successful retry", async () => {
    let shouldFail = true;
    const notifications: string[] = [];
    const harness = fakePi(async () => shouldFail
      ? { code: 1, killed: false, stderr: "pane unavailable" }
      : { code: 0, killed: false, stderr: "" });
    harness.install();

    await startTui(harness, notifications);
    await update(harness, "first", [running(0)]);
    expect(notifications).toEqual([
      "Herdr subagents: pane unavailable",
    ]);

    shouldFail = false;
    await update(harness, "retry", [running(0, { agent: "retry" })]);
    shouldFail = true;
    await update(harness, "fails-again", [running(0, { agent: "again" })]);
    expect(notifications).toEqual([
      "Herdr subagents: pane unavailable",
      "Herdr subagents: pane unavailable",
    ]);

    shouldFail = false;
    await harness.emit("session_shutdown");
  });

  test("serializes slow reports and publishes the newest queued state", async () => {
    const releases: Array<() => void> = [];
    const harness = fakePi(() => new Promise<ExecResult>((resolve) => {
      releases.push(() => resolve({ code: 0, killed: false, stderr: "" }));
    }));
    harness.install();

    const starting = harness.emit("session_start", {}, {
      mode: "tui",
      ui: { notify() {} },
    });
    await Bun.sleep(0);
    expect(harness.execCalls).toHaveLength(1);
    releases.shift()!();
    await starting;

    await harness.emit("tool_execution_start", {
      toolName: "subagent",
      toolCallId: "call",
    });
    await harness.emit("tool_execution_update", {
      toolName: "subagent",
      toolCallId: "call",
      partialResult: { details: { progress: [running(0, { agent: "first" })] } },
    });
    await waitForPublish();
    expect(harness.execCalls).toHaveLength(2);

    await harness.emit("tool_execution_update", {
      toolName: "subagent",
      toolCallId: "call",
      partialResult: { details: { progress: [running(0, { agent: "newest" })] } },
    });
    await waitForPublish();
    expect(harness.execCalls).toHaveLength(2);

    releases.shift()!();
    await Bun.sleep(0);
    expect(harness.execCalls).toHaveLength(3);
    expect(reportTokens(harness.execCalls[2].args).subagent_1).toBe(
      "|- newest / model-0",
    );

    releases.shift()!();
    await Bun.sleep(0);
    const stopping = harness.emit("session_shutdown");
    await Bun.sleep(0);
    releases.shift()!();
    await stopping;
  });
});
