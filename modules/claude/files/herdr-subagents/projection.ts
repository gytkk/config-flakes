export type Phase = "running" | "stopping" | "completed" | "failed" | "interrupted";
export type Child = {
  id: string;
  name: string;
  model?: string;
  modelAt?: number;
  activity: string;
  activityIsTool?: boolean;
  phase: Phase;
  startedAt: number;
  stoppedAt?: number;
  endedAt?: number;
  unavailable?: boolean;
  transcript?: string;
};
export type Event = {
  kind: "start" | "stop" | "result";
  id: string;
  at: number;
  name?: string;
  model?: string;
  activity?: string;
  phase?: Phase;
  transcript?: string;
};

export const validId = (value: unknown): value is string => typeof value === "string" && /^[a-zA-Z0-9_-]{1,128}$/.test(value);
export const object = (value: unknown): Record<string, any> | undefined => value !== null && typeof value === "object" && !Array.isArray(value) ? value as Record<string, any> : undefined;
export const text = (value: unknown): string => typeof value === "string"
  ? value.replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, "").replace(/[\x00-\x1f\x7f-\x9f]/g, " ").replace(/\s+/g, " ").trim() : "";
const blocks = (value: unknown): string => Array.isArray(value)
  ? value.filter((part) => part?.type === "text").map((part) => text(part.text)).filter(Boolean).join(" ") : text(value);
const terminal = (status: unknown): Phase | undefined => status === "completed" ? "completed" : status === "failed" ? "failed" : status === "killed" ? "interrupted" : undefined;

export function resultEvent(result: unknown, at: number, name?: unknown): Event | undefined {
  const value = object(result);
  if (!value || !validId(value.agentId)) return;
  const phase = value.status === "async_launched" ? "running" : terminal(value.status);
  if (!phase) return;
  const models = Array.isArray(value.modelsUsed) ? value.modelsUsed : [];
  return { kind: "result", id: value.agentId, at, phase, name: text(name ?? value.description),
    model: text(models.at(-1) ?? value.resolvedModel), activity: phase === "running" ? undefined : text(object(value.handbackReport)?.text) || blocks(value.content) };
}

export function hookEvent(payload: unknown, at = Date.now()): Event | undefined {
  const hook = object(payload);
  if (!hook) return;
  if (hook.hook_event_name === "SubagentStart" && validId(hook.agent_id)) {
    return { kind: "start", id: hook.agent_id, at, name: `${text(hook.agent_type) || "subagent"} (${hook.agent_id.slice(0, 6)})` };
  }
  if (hook.hook_event_name === "SubagentStop" && validId(hook.agent_id)) {
    return { kind: "stop", id: hook.agent_id, at, activity: text(hook.last_assistant_message), transcript: typeof hook.agent_transcript_path === "string" ? hook.agent_transcript_path : undefined };
  }
  if (hook.hook_event_name === "PostToolUse" && !hook.agent_id && hook.tool_name === "Agent") {
    return resultEvent(hook.tool_response, at, object(hook.tool_input)?.description);
  }
}

export function applyEvent(children: Map<string, Child>, event: Event): void {
  let child = children.get(event.id);
  if (!child) {
    // Internal Claude agents can emit stop hooks without a user-requested spawn.
    if (event.kind === "stop") return;
    child = { id: event.id, name: event.name || `subagent (${event.id.slice(0, 6)})`, activity: "", phase: "running", startedAt: event.at };
    children.set(event.id, child);
  }
  if (event.at < child.startedAt) return;
  if (event.transcript) child.transcript = event.transcript;
  if (event.kind === "start") {
    child.startedAt = event.at;
    child.phase = "running";
    child.activity = "";
    child.activityIsTool = false;
    child.stoppedAt = child.endedAt = undefined;
  } else if (event.kind === "stop") {
    if (child.endedAt !== undefined) return;
    child.stoppedAt = event.at;
    child.phase = "stopping";
    child.activity = event.activity || child.activity;
    if (event.activity) child.activityIsTool = false;
  } else {
    if (child.endedAt !== undefined && event.at <= child.endedAt) return;
    if (event.name) child.name = event.name;
    if (event.model && event.at >= (child.modelAt ?? 0)) { child.model = event.model; child.modelAt = event.at; }
    if (event.phase === "running") {
      if (child.endedAt !== undefined && event.at > child.endedAt) {
        child.phase = "running";
        child.startedAt = event.at;
        child.endedAt = child.stoppedAt = undefined;
        child.activity = "";
      }
      return;
    }
    child.phase = event.phase!;
    child.endedAt = event.at;
    child.activity = event.activity || child.activity;
    if (event.activity) child.activityIsTool = false;
  }
}

// Claude Code 2.1.284 stores child records with sessionId, agentId and message.
export function consumeChild(child: Child, value: unknown, sessionId: string): boolean {
  const record = object(value);
  if (!record || record.sessionId !== sessionId || record.agentId !== child.id || record.isSidechain !== true) return false;
  const at = Date.parse(record.timestamp);
  if (!Number.isFinite(at) || at < child.startedAt) return true;
  const message = object(record.message);
  if (child.endedAt !== undefined && ["user", "assistant"].includes(record.type)) {
    if (at <= child.endedAt) return true;
    child.phase = "running";
    child.startedAt = at;
    child.endedAt = child.stoppedAt = undefined;
    child.activity = "";
  }
  if (record.type === "assistant" && message) {
    if (text(message.model) && !text(message.model).startsWith("<") && at >= (child.modelAt ?? 0)) { child.model = text(message.model); child.modelAt = at; }
    const content = Array.isArray(message.content) ? message.content : [];
    const handback = content.find((part) => part.type === "tool_use" && part.name === "SubagentHandback");
    const activity = text(handback?.input?.message) || blocks(content);
    if (activity) { child.activity = activity; child.activityIsTool = false; }
    else if (!child.activity || child.activityIsTool) {
      const tool = content.find((part) => part.type === "tool_use");
      if (tool) { child.activity = `Using ${text(tool.name)}`; child.activityIsTool = true; }
    }
  }
  if (child.phase === "stopping" && at > child.stoppedAt! && ["user", "assistant"].includes(record.type)) {
    child.phase = "running";
    child.stoppedAt = undefined;
  }
  return true;
}

export function consumeParent(children: Map<string, Child>, value: unknown, sessionId: string, since: number, tasks: Map<string, string>): void {
  const record = object(value);
  if (!record || record.sessionId !== sessionId || record.isSidechain === true) return;
  const at = Date.parse(record.timestamp);
  if (!Number.isFinite(at) || at < since) return;
  const content = object(record.message)?.content;
  if (record.type === "assistant" && Array.isArray(content)) {
    for (const part of content) {
      if (part.type === "tool_use" && part.name === "Agent") tasks.set(part.id, text(part.input?.description));
    }
  }
  const result = object(record.toolUseResult);
  if (result) {
    const toolResult = Array.isArray(content) ? content.find((part) => part.type === "tool_result") : undefined;
    const event = resultEvent(result, at, tasks.get(toolResult?.tool_use_id));
    if (event) applyEvent(children, event);
  }
  if (record.type !== "user" || object(record.origin)?.kind !== "task-notification") return;
  const body = blocks(content);
  for (const match of body.matchAll(/<task-notification>(.*?)<\/task-notification>/g)) {
    const notification = match[1];
    const id = notification.match(/<task-id>([a-zA-Z0-9_-]+)<\/task-id>/)?.[1];
    const phase = terminal(notification.match(/<status>([^<]+)<\/status>/)?.[1]);
    const escaped = notification.match(/<result>(.*?)<\/result>/)?.[1];
    const activity = escaped?.replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&apos;/g, "'").replace(/&amp;/g, "&");
    if (id && phase && children.has(id)) applyEvent(children, { kind: "result", id, at, phase, activity });
  }
}

export function sidebarTokens(children: Child[], now = Date.now()): Record<string, string | null> {
  const active = children.filter((child) => child.endedAt === undefined
    && (child.stoppedAt === undefined || now - child.stoppedAt < 10_000));
  const ended = children.filter((child) => child.endedAt !== undefined && now >= child.endedAt && now - child.endedAt < 10_000)
    .sort((a, b) => b.endedAt! - a.endedAt!);
  const candidates = [...active, ...ended];
  const visible = candidates.length > 7 ? candidates.slice(0, 6) : candidates;
  const limit = (value: string) => { const chars = Array.from(value); return chars.length > 80 ? chars.slice(0, 79).join("") + "…" : chars.join(""); };
  const icons = { running: "●", stopping: "○", completed: "✓", failed: "×", interrupted: "■" };
  const tokens: Record<string, string | null> = {};
  for (let index = 0; index < 7; index++) {
    const child = visible[index];
    tokens[`subagent_${index + 1}_status`] = child ? child.unavailable ? "○" : icons[child.phase] : null;
    tokens[`subagent_${index + 1}`] = child ? limit(`${text(child.name)} / ${text(child.model) || "model unknown"}`) : null;
    const activity = text(child?.activity);
    tokens[`subagent_${index + 1}_activity`] = child && activity ? limit(`|  ${child.unavailable ? "Last message: " : ""}${activity}`) : null;
  }
  if (candidates.length > 7) {
    tokens.subagent_7_status = "…";
    tokens.subagent_7 = `+${candidates.length - visible.length} more`;
  }
  return tokens;
}
