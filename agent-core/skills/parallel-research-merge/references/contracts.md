# Worker contract and integration

Use this reference for an authorized investigation or implementation delegation. Reading it alone does not start the parallel research workflow. The runtime determines whether delegation is available and which execution mode is permitted.

## Before dispatch

Give each worker this compact contract, including only fields relevant to its task. Supply the actual facts and constraints, not placeholders or a pointer to a parent conversation the child cannot see.

```text
Task and outcome: <question to answer or behavior to implement; decision served>
Starting evidence: <relevant paths, confirmed facts, reproduction, or primary sources>
Allowed scope: <read-only by default; otherwise exclusive editable paths and exclusions>
Contracts to preserve: <existing behavior, approved design, compatibility, operating constraints>
Completion conditions: <observable result and required checks; analysis ends with findings>
Decision boundary: <local choices the worker owns; changes requiring parent approval>
Return: <conclusion or changed files; actual checks and outcomes; blockers and uncertainty>
```

## Investigation branch

Split by evidence that can be investigated independently, such as runtime installation state versus session usage, or backend behavior versus a separately specified client contract. Keep dependent questions sequential. The main agent retains a useful task and final decision ownership; a second worker is optional.

Include a finite source set, sample, check, or time limit, and a result budget appropriate to the question. Stop when the question is answered, the bound is reached, or blocking evidence is identified. Request decisive path:line or source evidence; link large artifacts only when requested.

Choose a concrete bound that can be checked. For example, inspect the two named runtime adapters and installed skill manifests, or sample the last 14 complete days from the named repository's root sessions. On a blocker, return the missing evidence and the smallest useful next step. Expand the scope only after the main agent decides the unanswered question still matters.

## Implementation branch

Provide the reproduction or expected behavior, owned paths, approved design, and exact acceptance checks where known. State which checks are authorized; task instructions do not make a command side-effect-free. Include relevant approval boundaries and user-work preservation rules when the child does not inherit them. A path to unreadable parent-only instructions is not a substitute.

The worker checks supplied evidence against the code, chooses implementation details within the agreed scope using existing conventions, and completes the change through the required verification. Changes to requirements, public APIs, dependencies, approved design, or edit ownership require the parent's decision and any required user approval before proceeding. On a blocker, return the evidence and smallest decision needed rather than broadening the task.

Return changed files, commands actually run and their outcomes, checks not run with reasons, and unresolved issues. Finish when the requested behavior and required checks are satisfied; additional investigation needs a concrete failure or unresolved risk.

## Reporting and ownership

Workers report what they verified and separate evidence from inference. Include changed files only when edits were allowed. Workers do not delegate further or commit unless the main agent explicitly assigns that responsibility.

## Editing and interruption

Permit worker edits only for a bounded change with exclusive file ownership and checks the main agent can review. Otherwise request findings and let the main agent implement. Reassign shared files sequentially when dependencies emerge.

Cancel or redirect work that duplicates verified findings, depends on unavailable upstream results, or no longer affects the decision. Reconcile any partial edits and worker status before finishing; a timeout is an unresolved result, not evidence that nothing was found.

## Integrating evidence

Verify claims that affect the conclusion, scope, or implementation. When workers disagree, compare the referenced versions, conditions, and checks; run the smallest discriminating check before choosing an answer. If the conflict cannot be resolved, preserve the uncertainty in the result.

Keep a short contribution note, using prose or a table as appropriate:

| Evidence | Disposition and reason | Effect on the task |
| --- | --- | --- |
| Source or check supporting a worker finding | Accepted, rejected, or unresolved, with the verification basis | Decision changed, check added, risk retained, or no additional value |

The note belongs in the existing working summary or final response, not a new repository report. Merge overlapping findings once. Inspect the final diff for authorized implementation, complete relevant checks, and account for every worker as completed, blocked, or cancelled.
