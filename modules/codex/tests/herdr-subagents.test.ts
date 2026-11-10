import { describe, expect, test } from "bun:test";
import {
  consumeChild,
  sidebarTokens,
  type ChildState,
} from "../files/herdr-subagents/projection";

function child(overrides: Partial<ChildState> = {}): ChildState {
  return {
    id: "thread-child-1",
    name: "reviewer",
    activity: "",
    observed: false,
    ...overrides,
  };
}

function event(payload: Record<string, unknown>, timestamp?: string) {
  return { type: "event_msg", timestamp, payload };
}

describe("Codex child activity projection", () => {
  test("keeps the latest assistant message while tools run and finish", () => {
    const state = child();
    consumeChild(state, event({ type: "task_started", turn_id: "turn-1" }));
    consumeChild(state, event({ type: "agent_message", message: "I found the relevant module." }));

    for (const payload of [
      { type: "exec_command_begin", command: "rg projection" },
      { type: "patch_apply_begin", changes: { "projection.ts": "update" } },
      { type: "mcp_tool_call_begin", server: "filesystem", tool: "read_file" },
      { type: "item_started", item: { type: "CommandExecution", command: "bun test" } },
      { type: "item_completed", item: { type: "CommandExecution", status: "completed" } },
      { type: "item_started", item: { type: "FileChange", changes: ["projection.ts"] } },
      { type: "item_completed", item: { type: "FileChange", status: "completed" } },
      { type: "item_started", item: { type: "McpToolCall", server: "filesystem", tool: "read_file" } },
      { type: "item_completed", item: { type: "McpToolCall", status: "completed" } },
    ]) {
      consumeChild(state, event(payload));
      expect(state.activity).toBe("I found the relevant module.");
    }

    expect(sidebarTokens([state]).subagent_1_activity).toBe("|  I found the relevant module.");
  });

  test("accepts the current turn's final message and rejects stale completion events", () => {
    const state = child();
    consumeChild(state, event({ type: "task_started", turn_id: "turn-current" }));
    consumeChild(state, event({ type: "agent_message", message: "Draft result" }));

    expect(consumeChild(state, event({
      type: "task_complete",
      turn_id: "turn-old",
      last_agent_message: "Stale final result",
      completed_at: 2_000,
    }))).toBeFalse();
    expect(state).toMatchObject({ phase: "running", activity: "Draft result" });

    expect(consumeChild(state, event({
      type: "task_complete",
      turn_id: "turn-current",
      last_agent_message: "Final result",
      completed_at: 2_001,
    }))).toBeTrue();
    expect(state).toMatchObject({ phase: "completed", activity: "Final result" });

    consumeChild(state, event({ type: "task_started", turn_id: "turn-follow-up" }));
    consumeChild(state, event({ type: "agent_message", message: "Follow-up progress" }));
    consumeChild(state, event({
      type: "task_failed",
      turn_id: "turn-follow-up",
      last_agent_message: "Failure metadata must not replace the message",
      completed_at: 2_002,
    }));
    expect(state).toMatchObject({ phase: "failed", activity: "Follow-up progress" });
  });

  test("hides empty follow-up activity and preserves the terminal visibility window", () => {
    const state = child();
    consumeChild(state, event({ type: "task_started", turn_id: "turn-1" }));
    consumeChild(state, event({ type: "agent_message", message: "First turn done" }));
    consumeChild(state, event({
      type: "task_complete",
      turn_id: "turn-1",
      completed_at: 10,
    }));

    expect(sidebarTokens([state], 14_999)).toMatchObject({
      subagent_1_status: "✓",
      subagent_1: "reviewer / model unknown",
      subagent_1_activity: "|  First turn done",
    });
    expect(sidebarTokens([state], 15_000).subagent_1).toBeNull();

    consumeChild(state, event({ type: "task_started", turn_id: "turn-2" }));
    expect(state).toMatchObject({ phase: "running", activity: "" });
    expect(sidebarTokens([state], 15_000)).toMatchObject({
      subagent_1_status: "●",
      subagent_1: "reviewer / model unknown",
      subagent_1_activity: null,
    });
  });

  test("sanitizes activity and limits the full sidebar line to 80 Unicode codepoints", () => {
    const state = child();
    consumeChild(state, event({ type: "task_started", turn_id: "turn-1" }));
    state.observed = false;
    consumeChild(state, event({
      type: "agent_message",
      message: "\u001b[31m  hello\n\tworld  \u0007\u001b[0m",
    }));
    expect(sidebarTokens([state]).subagent_1_activity).toBe("|  Last message: hello world");

    state.activity = "한".repeat(63);
    const exact = sidebarTokens([state]).subagent_1_activity!;
    expect(Array.from(exact)).toHaveLength(80);
    expect(exact).not.toEndWith("…");

    state.activity = "🙂".repeat(100);
    const truncated = sidebarTokens([state]).subagent_1_activity!;
    expect(Array.from(truncated)).toHaveLength(80);
    expect(truncated).toStartWith("|  Last message: ");
    expect(truncated).toEndWith("…");
    expect(sidebarTokens([state])).not.toHaveProperty("subagent_count");
  });

  test("reads supported assistant message shapes and ignores user and reasoning items", () => {
    const state = child();
    consumeChild(state, {
      type: "response_item",
      payload: {
        type: "message",
        role: "assistant",
        content: [{ type: "output_text", text: "First" }, { type: "output_text", text: "answer" }],
      },
    });
    expect(state.activity).toBe("First answer");

    consumeChild(state, {
      type: "response_item",
      payload: {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "User text" }],
      },
    });
    consumeChild(state, {
      type: "response_item",
      payload: {
        type: "reasoning",
        content: [{ type: "reasoning_text", text: "Private reasoning" }],
      },
    });
    expect(state.activity).toBe("First answer");

    consumeChild(state, event({
      type: "item_started",
      item: { type: "AgentMessage", content: [{ type: "output_text", text: "Streaming update" }] },
    }));
    expect(state.activity).toBe("Streaming update");

    consumeChild(state, event({
      type: "item_completed",
      item: {
        type: "AgentMessage",
        content: [{ type: "output_text", text: "Completed" }, { type: "output_text", text: "answer" }],
      },
    }));
    expect(state.activity).toBe("Completed answer");

    consumeChild(state, event({
      type: "item_completed",
      item: { type: "AgentMessage", content: [{ type: "output_text", text: " \n " }] },
    }));
    expect(state.activity).toBe("Completed answer");
  });
});
