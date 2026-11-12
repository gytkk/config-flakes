import { acceptanceEvidence, workerCompletion, type AcceptanceEvidence, type WorkerCompletion } from "./completion";

// Public pi-subagents 0.84 Details/SingleResult projections; no package imports.
export type Owner = { sessionId: string; sessionFile: string | null };
export type Target = { runId: string; index: number };
export type Child = {
  runId: string;
  index: number;
  agent: string;
  task: string;
  model: string;
  status: string;
  result: string;
  sessionFile?: string;
  source: "foreground" | "hub-resume";
  previous?: Target;
  root?: Target;
  evidence?: "foreground" | "foreground-progress" | "receipt" | "snapshot" | "completion";
  completion?: WorkerCompletion;
  completionInvalid?: boolean;
  acceptance?: AcceptanceEvidence;
};
export const OBSERVATION = "agent-hub-observation-v1";
export const RESUME = "agent-hub-resume-v1";
export type ResumeReceipt = {
  owner: Owner;
  requestId: string;
  target: Child;
  followUp?: string;
  state: "requested" | "accepted" | "unknown" | "rejected";
  resumedRunId?: string;
  error?: string;
};

export function record(value: unknown): Record<string, unknown> | undefined {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown> : undefined;
}
export function identity(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 && value.length <= 1024 && !/[\x00-\x20\x7f-\x9f]/.test(value)
    ? value : undefined;
}
export function sameOwner(value: unknown, owner: Owner): boolean {
  const candidate = record(value);
  return candidate?.sessionId === owner.sessionId && candidate?.sessionFile === owner.sessionFile;
}
export function childKey(child: Pick<Child, "runId" | "index">): string {
  return JSON.stringify([child.runId, child.index]);
}
const string = (value: unknown) => typeof value === "string" ? value : "";

export const TASK_UNAVAILABLE = "Unavailable: package redacted the task and no unambiguous originating call is retained.";

// Caller-facing tasks only; never interpret chain templates or row position as identity.
export function callerTask(input: unknown, index: number, agent: string): string | undefined {
  const args = record(input);
  if (!args || args.chain || args.workflow || args.workflowScript || args.action) return;
  const options = record(args.options);
  if (args.count !== undefined || options?.count !== undefined) return;
  if (Array.isArray(args.tasks)) {
    let offset = 0;
    for (const raw of args.tasks) {
      const task = record(raw);
      const count = task?.count ?? 1;
      if (!task || !Number.isSafeInteger(count) || (count as number) < 1 || (count as number) > 256) return;
      if (index >= offset && index < offset + (count as number)) return task.agent === agent ? string(task.task) || undefined : undefined;
      offset += count as number;
    }
    return;
  }
  return index === 0 && args.agent === agent ? string(args.task) || undefined : undefined;
}

export function branchOrigins(entries: readonly unknown[]) {
  const tools = new Map<string, unknown>();
  const slash = new Map<string, unknown>();
  for (const raw of entries) {
    const entry = record(raw);
    const message = record(entry?.message);
    if (entry?.type === "message" && message?.role === "assistant" && Array.isArray(message.content)) {
      for (const part of message.content) {
        const call = record(part);
        if (call?.type === "toolCall" && call.name === "subagent" && identity(call.id)) tools.set(call.id as string, call.arguments);
      }
    }
    const wrapper = record(entry?.details);
    const details = record(record(wrapper?.result)?.details);
    if (entry?.type === "custom_message" && entry.customType === "subagent-slash-result" && identity(wrapper?.requestId) && details && !details.runId && !details.asyncId) {
      // 0.84 slash placeholders do not expand counts. Only single index 0 is unambiguous.
      const rows = Array.isArray(details.results) ? details.results : [];
      const row = record(rows[0]);
      if (!slash.has(wrapper!.requestId as string)) slash.set(wrapper!.requestId as string, details.mode === "single" && rows.length === 1 && row?.index === 0 ? { agent: row.agent, task: row.task } : undefined);
    }
  }
  return { tools, slash };
}

export function foregroundRows(value: unknown, terminal: boolean, origin?: unknown): Child[] {
  const details = record(value);
  const runId = identity(details?.runId);
  if (!runId || details?.background === true || details?.asyncId || !["single", "parallel", "chain"].includes(string(details?.mode))) return [];
  if (!Array.isArray(details?.results) || (details.mode === "single" && details.results.length > 1)) return [];
  const indexes = details.results.map(raw => record(raw)?.index);
  const rows: Child[] = [];
  for (const raw of details.results) {
    const row = record(raw);
    if (!row || !Number.isSafeInteger(row.index) || (row.index as number) < 0 || !identity(row.agent)) continue;
    // Native foreground children have no external runner. Do not promote nested/workflow rows.
    if (row.runner || row.workflowKey || row.runId || indexes.filter(index => index === row.index).length !== 1) continue;
    const progress = record(row.progress);
    const status = row.detached === true ? "detached" : row.stopped === true ? "stopped"
      : row.timedOut === true ? "timed out" : row.interrupted === true ? "interrupted"
      : terminal ? row.exitCode === 0 ? "completed" : typeof row.exitCode === "number" ? "failed" : "unknown"
      : string(progress?.status) || "running";
    rows.push({
      runId, index: row.index as number, agent: row.agent as string,
      task: callerTask(origin, row.index as number, row.agent as string) ?? (string(row.task) && row.task !== "[prompt redacted]" ? string(row.task) : TASK_UNAVAILABLE), model: string(row.model) || string(progress?.model) || "unknown",
      status, result: string(row.finalOutput) || string(row.error),
      sessionFile: typeof row.sessionFile === "string" ? row.sessionFile : undefined,
      source: "foreground", evidence: terminal ? "foreground" : "foreground-progress",
      completion: terminal && row.structuredOutputFailed !== true ? workerCompletion(row.structuredOutput) : undefined,
      completionInvalid: terminal && (row.structuredOutputFailed === true || (row.structuredOutput !== undefined && !workerCompletion(row.structuredOutput))),
      acceptance: terminal ? acceptanceEvidence(row.acceptance) : undefined,
    });
  }
  return rows;
}

export function restoreRecords(entries: readonly unknown[], owner: Owner, forked: boolean): {
  children: Map<string, Child>; receipts: Map<string, ResumeReceipt>;
} {
  const children = new Map<string, Child>();
  const receipts = new Map<string, ResumeReceipt>();
  const observed = new Set<string>();
  const origins = branchOrigins(entries);
  for (const raw of entries) {
    const entry = record(raw);
    const data = record(entry?.data);
    if (entry?.type === "custom" && entry.customType === OBSERVATION && sameOwner(data?.owner, owner)) {
      const child = parseChild(data?.child);
      if (child?.source === "foreground" && child.runId === data?.runId) observed.add(childKey(child));
    }
  }
  for (const raw of entries) {
    const entry = record(raw);
    const message = record(entry?.message);
    const details = entry?.type === "message" && message?.role === "toolResult" && message.toolName === "subagent"
      ? message.details
      : entry?.type === "custom_message" && entry.customType === "subagent-slash-result"
        ? record(record(entry.details)?.result)?.details : undefined;
    const toolId = identity(message?.toolCallId);
    const slashId = identity(record(entry?.details)?.requestId);
    const hasOrigin = toolId ? origins.tools.has(toolId) : slashId ? origins.slash.has(slashId) : false;
    const origin = toolId ? origins.tools.get(toolId) : slashId ? origins.slash.get(slashId) : undefined;
    for (const child of foregroundRows(details, true, origin)) {
      if (observed.has(childKey(child)) || (!forked && hasOrigin)) children.set(childKey(child), mergeChild(children.get(childKey(child)), child));
    }
    const data = record(entry?.data);
    if (entry?.type === "custom" && entry.customType === OBSERVATION && sameOwner(data?.owner, owner)) {
      const child = parseChild(data?.child);
      if (child?.source === "foreground" && child.runId === data?.runId) children.set(childKey(child), mergeChild(children.get(childKey(child)), child));
    }
    if (entry?.type !== "custom" || entry.customType !== RESUME || !sameOwner(data?.owner, owner)) continue;
    const target = record(data?.target);
    if (!identity(data?.requestId) || !identity(target?.runId) || !Number.isSafeInteger(target?.index) || (target!.index as number) < 0 || !identity(target?.agent)) continue;
    if (!["task", "model", "status", "result"].every(field => typeof target?.[field] === "string")) continue;
    if (target?.source !== "foreground" && target?.source !== "hub-resume") continue;
    if (!["requested", "accepted", "unknown", "rejected"].includes(string(data?.state))) continue;
    const receipt = data as unknown as ResumeReceipt;
    receipts.set(childKey(receipt.target), receipt);
    if (receipt.state === "accepted" && identity(receipt.resumedRunId)) {
      const previous = { runId: receipt.target.runId, index: receipt.target.index };
      const child: Child = { ...receipt.target, task: typeof receipt.followUp === "string" ? receipt.followUp : receipt.target.task, runId: receipt.resumedRunId!, index: 0, status: "accepted", result: "Resume accepted; completion is not yet established by this receipt.", source: "hub-resume", sessionFile: undefined, completion: undefined, completionInvalid: false, acceptance: undefined, previous, root: receipt.target.root ?? previous, evidence: "receipt" };
      children.set(childKey(child), child);
    }
  }
  // A completion may be delivered before the resume RPC reply. Establish exact
  // owner receipts first, then apply observations only to those accepted targets.
  for (const raw of entries) {
    const entry = record(raw);
    const data = record(entry?.data);
    if (entry?.type === "custom" && entry.customType === OBSERVATION && sameOwner(data?.owner, owner)) {
      const update = parseChild(data?.child);
      const child = update && children.get(childKey(update));
      if (child?.source === "hub-resume" && update?.source === "hub-resume") children.set(childKey(child), mergeChild(child, { ...child, status: update.status, model: update.model, result: update.result, sessionFile: update.sessionFile, evidence: update.evidence, completion: update.completion, completionInvalid: update.completionInvalid, acceptance: update.acceptance }));
    }
    if (entry?.type === "custom_message" && entry.customType === "subagent-notify") {
      for (const [key, child] of children) {
        const update = completionObservation(child, entry.details);
        if (update) children.set(key, mergeChild(child, update));
      }
    }
  }
  return { children, receipts };
}

function parseChild(value: unknown): Child | undefined {
  const child = record(value);
  if (!identity(child?.runId) || !Number.isSafeInteger(child?.index) || (child!.index as number) < 0 || !identity(child?.agent)) return;
  if (!["task", "model", "status", "result"].every(key => typeof child?.[key] === "string")) return;
  if (child?.source !== "foreground" && child?.source !== "hub-resume") return;
  return { ...child, completion: workerCompletion(child.completion), completionInvalid: child.completionInvalid === true || (child.completion !== undefined && !workerCompletion(child.completion)), acceptance: acceptanceEvidence(child.acceptance) } as Child;
}

export function terminalStatus(status: string): boolean {
  return ["completed", "failed", "paused", "stopped", "interrupted", "timed out", "rejected", "partial"].includes(status);
}

export function mergeChild(previous: Child | undefined, next: Child): Child {
  if (!previous || childKey(previous) !== childKey(next)) return next;
  // Revival has a NEW identity. Stopping seals this exact run, including when
  // a delayed terminal snapshot disagrees with the richer native notice.
  if (previous.status === "stopped") return previous;
  if (next.status === "stopped") return next;
  if (terminalStatus(previous.status) && !terminalStatus(next.status)) return previous;
  if (previous.evidence === "completion" && next.evidence === "snapshot") return previous;
  if (previous.evidence === "foreground" && terminalStatus(previous.status) && next.evidence === "foreground-progress") return previous;
  return { ...next, task: next.task === TASK_UNAVAILABLE ? previous.task : next.task };
}

// Native single revival returns executeAsyncSingle's empty results receipt.
// Composite, conflicting or unidentified receipts cannot authorize index zero.
export function resumedRun(response: Record<string, unknown>): string | undefined {
  const details = record(response.details);
  const id = identity(details?.asyncId);
  if (response.isError === true || !id || details?.mode !== "single" || !Array.isArray(details.results) || details.results.length !== 0) return;
  if (details.runId !== undefined && details.runId !== id) return;
  if (details.parentWorkflowRunId || details.workflowKey) return;
  return id;
}

export function snapshotObservation(child: Child, value: unknown): Child | undefined {
  if (child.source !== "hub-resume" || child.index !== 0) return;
  const snapshot = record(value);
  if (snapshot?.kind !== "pi-subagents.async-status-snapshot" || snapshot.version !== 1 || !Array.isArray(snapshot.runs)) return;
  const matches = snapshot.runs.map(record).filter(run => run?.id === child.runId);
  if (matches.length !== 1) return;
  const run = matches[0]!;
  if (run.kind !== "subagent" || !["queued", "running", "complete", "failed", "partial", "paused", "stopped", "rejected"].includes(string(run.state))) return;
  const steps = Array.isArray(run.children) ? run.children.map(record).filter(step => step?.kind === "step") : [];
  // Read only the exact root and step:0; never walk nested children or Fleet keys.
  const step = steps.length === 1 && steps[0]?.id === "step:0" ? steps[0] : undefined;
  if (steps.length && !step) return;
  return { ...child, status: run.state === "complete" ? "completed" : run.state as string, model: string(step?.model) || child.model, evidence: "snapshot", result: `Package snapshot: ${run.state}. Read package status or transcript for output.` };
}

export function completionObservation(child: Child, value: unknown): Child | undefined {
  if (child.source !== "hub-resume" || child.index !== 0) return;
  const details = record(value);
  if (!Array.isArray(details?.runs)) return;
  const matches = details.runs.map(record).filter(run => run?.runId === child.runId);
  if (matches.length !== 1) return;
  const run = matches[0]!;
  if (run.workflowRunId || run.source === "foreground" || !["completed", "failed", "paused", "stopped"].includes(string(run.status))) return;
  return { ...child, status: run.status as string, evidence: "completion", result: `Package completion notice: ${run.status}. Read the transcript for output.`, ...(run.sessionLabel === "Session file" && typeof run.sessionValue === "string" ? { sessionFile: run.sessionValue } : {}) };
}
