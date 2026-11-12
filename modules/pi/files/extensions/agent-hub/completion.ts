// Display projections only: worker claims and package host evidence never
// authorize execution or become a combined "success" verdict.
export type WorkerCompletion = {
  contract: "pi-worker-completion-v1";
  status: "completed" | "blocked" | "failed";
  checks: Array<{ command: string; result: "passed" | "failed" | "not-run"; evidence: string }>;
  incomplete: string[];
  summary?: string;
};
export type AcceptanceEvidence = {
  status: string;
  evidenceStatus: string;
  runtimeChecks: Array<{ id: string; status: string; message: string }>;
  verifyRuns: Array<{ command: string; status: string; exitCode: number | null; memoized?: boolean }>;
};
const object = (value: unknown): Record<string, unknown> | undefined => value !== null && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : undefined;
// JSON Schema maxLength counts Unicode code points, not UTF-16 code units.
const text = (value: unknown, max = 1000): value is string => typeof value === "string" && value.length <= max * 2 && Array.from(value).length <= max && /\S/.test(value);
const oneOf = (value: unknown, choices: string[]) => typeof value === "string" && choices.includes(value);
const list = (value: unknown): value is unknown[] => Array.isArray(value) && value.length <= 16;
const keys = (value: Record<string, unknown>, allowed: string[]) => Object.keys(value).every(key => allowed.includes(key));

// Mirrors the versioned schema in modules/pi/agents/worker.md. Parity cases
// exercise both this recognizer and the installed package's schema validator.
export function workerCompletion(value: unknown): WorkerCompletion | undefined {
  const data = object(value);
  if (!data || data.contract !== "pi-worker-completion-v1" || !keys(data, ["contract", "status", "checks", "incomplete", "summary"])) return;
  if (!oneOf(data.status, ["completed", "blocked", "failed"]) || !list(data.checks) || !list(data.incomplete)) return;
  if (data.summary !== undefined && !text(data.summary, 2000)) return;
  if (!data.incomplete.every(item => text(item)) || (data.status === "completed" && data.incomplete.length)) return;
  if (!data.checks.every(item => {
    const check = object(item);
    return check && keys(check, ["command", "result", "evidence"]) && text(check.command) && text(check.evidence) && oneOf(check.result, ["passed", "failed", "not-run"]);
  })) return;
  return structuredClone(data) as WorkerCompletion;
}

// Accept only the public SingleResult.acceptance ledger fields, never the
// childReport's similarly named self-reported commands. No commands run here.
export function acceptanceEvidence(value: unknown): AcceptanceEvidence | undefined {
  const data = object(value);
  const evidenceStatuses = ["pending", "not-required", "claimed", "attested", "checked", "verified", "rejected"];
  if (!data || !oneOf(data.status, [...evidenceStatuses, "review-required", "reviewed", "accepted"]) || !oneOf(data.evidenceStatus, evidenceStatuses)) return;
  if (!list(data.runtimeChecks) || !list(data.verifyRuns)) return;
  const runtimeChecks: AcceptanceEvidence["runtimeChecks"] = [];
  const verifyRuns: AcceptanceEvidence["verifyRuns"] = [];
  for (const raw of data.runtimeChecks) {
    const check = object(raw);
    if (!check || !text(check.id) || !text(check.message) || !oneOf(check.status, ["passed", "failed", "not-applicable"])) return;
    runtimeChecks.push({ id: check.id, status: check.status as string, message: check.message });
  }
  for (const raw of data.verifyRuns) {
    const run = object(raw);
    if (!run || !text(run.command) || !oneOf(run.status, ["passed", "failed", "timed-out", "allowed-failure"]) || !(run.exitCode === null || Number.isSafeInteger(run.exitCode))) return;
    if (run.memoized !== undefined && typeof run.memoized !== "boolean") return;
    verifyRuns.push({ command: run.command, status: run.status as string, exitCode: run.exitCode as number | null, ...(typeof run.memoized === "boolean" ? { memoized: run.memoized } : {}) });
  }
  return { status: data.status as string, evidenceStatus: data.evidenceStatus as string, runtimeChecks, verifyRuns };
}

export function completionText(child: { completion?: WorkerCompletion; completionInvalid?: boolean; acceptance?: AcceptanceEvidence }): string {
  const verdict = workerCompletion(child.completion);
  const acceptance = acceptanceEvidence(child.acceptance);
  return [
    `Worker verdict: ${verdict ? `${verdict.status} (self-reported)` : child.completionInvalid ? "unknown (invalid or unrecognized structured output)" : "unknown (no recognized structured evidence)"}`,
    ...(verdict ? [`Worker check evidence (self-reported)\n${JSON.stringify(verdict.checks, null, 2)}`, `Incomplete required work\n${JSON.stringify(verdict.incomplete)}`, ...(verdict.summary ? [`Worker summary\n${verdict.summary}`] : [])] : []),
    acceptance ? `Package acceptance: ${acceptance.status}; evidence: ${acceptance.evidenceStatus}\nHost structural checks\n${JSON.stringify(acceptance.runtimeChecks, null, 2)}\nHost verification commands (memoized: true means reused evidence)\n${JSON.stringify(acceptance.verifyRuns, null, 2)}` : "Package acceptance/host checks: unknown (no recognized ledger)",
    "A worker verdict is not host verification. Structural acceptance is not test correctness; host commands prove only their recorded checks. Hub runs no verifier.",
  ].join("\n\n");
}
