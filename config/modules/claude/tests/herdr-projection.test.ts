import { describe, expect, test } from "bun:test";
import { applyEvent, consumeChild, consumeParent, hookEvent, resultEvent, sidebarTokens, type Child } from "../files/herdr-subagents/projection";

const sessionId = "parent";
const stamp = (at: number) => new Date(at).toISOString();
function running(at = 1000) {
  const children = new Map<string, Child>();
  applyEvent(children, { kind: "start", id: "child", name: "Explore (child)", at });
  return { children, child: children.get("child")! };
}
function assistant(at: number, content: unknown[], extra = {}) {
  return { type: "assistant", agentId: "child", sessionId, isSidechain: true, timestamp: stamp(at), message: { model: "claude-test", content }, ...extra };
}
function parent(at: number, extra: object) {
  return { sessionId, timestamp: stamp(at), ...extra };
}

describe("Claude child projection", () => {
  test("uses stable hook IDs, ignores stop-only internal agents and nested Agent results", () => {
    expect(hookEvent({ hook_event_name: "SubagentStart", agent_id: "../bad" })).toBeUndefined();
    const start = hookEvent({ hook_event_name: "SubagentStart", agent_id: "abcdef123", agent_type: "Explore" }, 1000)!;
    const children = new Map<string, Child>();
    applyEvent(children, { kind: "stop", id: "internal", at: 1000 });
    applyEvent(children, start);
    expect([...children.keys()]).toEqual(["abcdef123"]);
    expect(children.get(start.id)?.name).toBe("Explore (abcdef)");
    expect(hookEvent({ hook_event_name: "PostToolUse", agent_id: "nested", tool_name: "Agent", tool_response: { agentId: "other", status: "completed" } })).toBeUndefined();
  });

  test("reports actual model and text while ignoring thinking, tool results and foreign children", () => {
    const { child } = running();
    consumeChild(child, assistant(1001, [{ type: "thinking", thinking: "private reasoning" }, { type: "tool_use", name: "Read", input: { file_path: "private-path" } }]), sessionId);
    expect(child).toMatchObject({ model: "claude-test", activity: "Using Read" });
    consumeChild(child, assistant(1002, [{ type: "tool_use", name: "Bash" }]), sessionId);
    expect(child.activity).toBe("Using Bash");
    consumeChild(child, assistant(1002, [{ type: "text", text: "Checking the module." }]), sessionId);
    consumeChild(child, assistant(1003, [{ type: "tool_use", name: "Bash", input: { command: "private command" } }]), sessionId);
    expect(consumeChild(child, assistant(1004, [{ type: "text", text: "Foreign" }], { agentId: "foreign" }), sessionId)).toBeFalse();
    expect(consumeChild(child, assistant(1004, [{ type: "text", text: "Wrong session" }], { sessionId: "foreign" }), sessionId)).toBeFalse();
    expect(child.activity).toBe("Checking the module.");
    expect(child.phase).toBe("running");
  });

  test("does not infer completion from end_turn or stop hooks, and handles blocked stopping", () => {
    const { children, child } = running();
    const response = assistant(1100, [{ type: "text", text: "First answer" }]);
    consumeChild(child, { ...response, message: { ...response.message, stop_reason: "end_turn" } }, sessionId);
    expect(child.phase).toBe("running");
    applyEvent(children, hookEvent({ hook_event_name: "SubagentStop", agent_id: "child", last_assistant_message: "First answer" }, 1200)!);
    expect(sidebarTokens([child], 2000).subagent_1_status).toBe("○");
    consumeChild(child, { type: "user", sessionId, agentId: "child", isSidechain: true, timestamp: stamp(1300), message: { content: "Hook asked for more work" } }, sessionId);
    expect(child.phase).toBe("running");
    expect(child.endedAt).toBeUndefined();
  });

  test("maps parent Agent results to descriptions, models and confirmed completion", () => {
    const { children, child } = running();
    const tasks = new Map<string, string>();
    consumeParent(children, parent(1050, { type: "assistant", message: { content: [{ type: "tool_use", id: "tool-1", name: "Agent", input: { description: "Review hooks" } }] } }), sessionId, 1000, tasks);
    consumeParent(children, parent(2000, { type: "user", message: { content: [{ type: "tool_result", tool_use_id: "tool-1" }] }, toolUseResult: { status: "completed", agentId: "child", resolvedModel: "first-model", modelsUsed: ["first-model", "final-model"], content: [{ type: "text", text: "Done" }] } }), sessionId, 1000, tasks);
    expect(child).toMatchObject({ name: "Review hooks", phase: "completed", model: "final-model", activity: "Done", endedAt: 2000 });
    consumeChild(child, assistant(1500, [{ type: "text", text: "Old progress" }]), sessionId);
    expect(child.activity).toBe("Done");
    expect(sidebarTokens([child], 11999).subagent_1_status).toBe("✓");
    expect(sidebarTokens([child], 12000).subagent_1).toBeNull();
  });

  test("hides stop-only children after ten seconds and shows resumed activity again", () => {
    const { children, child } = running();
    applyEvent(children, hookEvent({ hook_event_name: "SubagentStop", agent_id: "child" }, 2000)!);
    expect(sidebarTokens([child], 11999).subagent_1_status).toBe("○");
    expect(sidebarTokens([child], 12000)).toMatchObject({ subagent_1: null, subagent_1_status: null, subagent_1_activity: null });
    expect(child.phase).toBe("stopping");
    consumeChild(child, assistant(13000, [{ type: "text", text: "More work" }]), sessionId);
    expect(sidebarTokens([child], 13000)).toMatchObject({ subagent_1_status: "●", subagent_1_activity: "|  More work" });
  });

  test("expired stops do not crowd out running children or count toward overflow", () => {
    const children = Array.from({ length: 8 }, (_, index) => ({ ...running().child,
      id: String(index), phase: "stopping" as const, stoppedAt: 2000 }));
    const active = running().child;
    active.unavailable = true;
    expect(sidebarTokens([...children, active], 12000)).toMatchObject({
      subagent_1: "Explore (child) / model unknown", subagent_2: null, subagent_7: null,
    });
  });

  test("background launches stay active and handback reports use their text field", () => {
    const { children, child } = running();
    const launch = hookEvent({ hook_event_name: "PostToolUse", tool_name: "Agent", tool_input: { description: "Inspect lifecycle" }, tool_response: { status: "async_launched", agentId: "child", resolvedModel: "claude-background" } }, 1100)!;
    applyEvent(children, launch);
    expect(child).toMatchObject({ phase: "running", name: "Inspect lifecycle", model: "claude-background" });
    consumeChild(child, assistant(1200, [{ type: "tool_use", name: "SubagentHandback", input: { message: "Delivered report" } }]), sessionId);
    expect(child.activity).toBe("Delivered report");
    const done = resultEvent({ status: "completed", agentId: "child", resolvedModel: "claude-background", modelsUsed: ["claude-background", "claude-final"], content: [{ type: "text", text: "See handback" }], handbackReport: { text: "Actual report" } }, 1300)!;
    applyEvent(children, done);
    expect(child.activity).toBe("Actual report");
    expect(child.model).toBe("claude-final");
    applyEvent(children, { kind: "result", id: "child", at: 1100, model: "stale", phase: "running" });
    expect(child.model).toBe("claude-final");
  });

  test("only trusted notifications change lifecycle; supports wrapped results and failure states", () => {
    for (const [status, expected] of [["completed", "completed"], ["failed", "failed"], ["killed", "interrupted"]]) {
      const { children, child } = running();
      const message = { content: `<system-reminder><task-notification><task-id>child</task-id><status>${status}</status><result>A &lt; B &amp; C</result></task-notification></system-reminder>` };
      consumeParent(children, parent(2000, { type: "user", message }), sessionId, 1000, new Map());
      expect(child.phase).toBe("running");
      consumeParent(children, parent(2100, { type: "user", message, origin: { kind: "task-notification" } }), sessionId, 1000, new Map());
      expect(child).toMatchObject({ phase: expected, activity: "A < B & C" });
    }
  });

  test("resumes the same ID without accepting stale completion or reviving old history", () => {
    const { children, child } = running();
    applyEvent(children, { kind: "result", id: "child", at: 2000, phase: "completed" });
    applyEvent(children, { kind: "start", id: "child", at: 3000 });
    applyEvent(children, { kind: "result", id: "child", at: 2001, phase: "completed" });
    consumeChild(child, assistant(1500, [{ type: "text", text: "Old" }]), sessionId);
    expect(child).toMatchObject({ phase: "running", activity: "", startedAt: 3000 });
    expect(children.size).toBe(1);
    applyEvent(children, { kind: "result", id: "child", at: 4000, phase: "completed" });
    consumeChild(child, assistant(5000, [{ type: "text", text: "Resumed without a start hook" }]), sessionId);
    expect(child).toMatchObject({ phase: "running", activity: "Resumed without a start hook", startedAt: 5000 });
    const empty = new Map<string, Child>();
    consumeParent(empty, parent(500, { type: "user", toolUseResult: { status: "async_launched", agentId: "old" } }), sessionId, 1000, new Map());
    expect(empty.size).toBe(0);
  });

  test("sanitizes and bounds text, shows overflow, and retains unknown last messages", () => {
    const { child } = running();
    child.activity = "\u001b[31mhello\n" + "한".repeat(100);
    child.unavailable = true;
    const line = sidebarTokens([child], 2000).subagent_1_activity!;
    expect(Array.from(line)).toHaveLength(80);
    expect(line).toStartWith("|  Last message: hello ");
    expect(line).toEndWith("…");
    expect(line).not.toContain("\u001b");
    expect(sidebarTokens([child], 2000).subagent_1_status).toBe("○");
    const children = Array.from({ length: 9 }, (_, index) => ({ ...child, id: String(index) }));
    expect(sidebarTokens(children, 2000)).toMatchObject({ subagent_7_status: "…", subagent_7: "+3 more", subagent_7_activity: null });
    expect(sidebarTokens([], 2000).subagent_1).toBeNull();
  });
});
