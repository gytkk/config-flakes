---
name: parallel-research-merge
description: Reconcile independent investigations that can change a design, diagnosis, or recommendation while the main agent has useful concurrent work. Use for explicit parallel research requests; not routine implementation delegation, single-source research, or dependent lookups.
---

# Parallel research merge

## Decide whether to delegate

Identify the unresolved decision, the independent question a worker can answer, and the useful work the main agent will do while waiting. Delegate when that answer could change the decision or its verification and the expected benefit justifies dispatch, context gathering, and review.

The main agent plus one worker is sufficient. Add another worker only for a separate unresolved question with disjoint evidence or file ownership. A task's size, multiple files, or a request mentioning agents does not by itself justify parallel research.

Keep routine edits, dependent lookups, and work whose answer is already known in the main thread. When the user explicitly requests parallel research, honor that scope where separable work exists; explain a sequential dependency when it prevents useful parallelism. If worker tools are unavailable, investigate only the necessary questions sequentially without reproducing worker reports.

## Investigate and reconcile

1. Keep scope and final decisions with the main agent. Before an authorized dispatch, read [the worker contract and investigation branch](references/contracts.md) and specify the decision served, evidence scope, edit ownership, investigation bound, and expected result size. Default workers to read-only investigation. Follow the runtime's execution limits; when concurrent children are unavailable, investigate sequentially rather than changing execution mode.
2. Dispatch only independent questions and continue the main agent's work. Share new constraints promptly. Reuse a worker for a related follow-up instead of starting overlapping investigations.
3. Verify decision-changing claims against cited sources or checks. Resolve conflicting evidence before making dependent changes; agreement alone is not verification. Stop an investigation when it answers its question, reaches its bound, or becomes irrelevant. Close or cancel every dispatched unit before finishing.
4. Record a compact contribution note in the working summary: evidence accepted or rejected, the reason, and the resulting decision or verification change. If a worker added no useful evidence, record that instead of inventing a benefit. Use the note to avoid redundant follow-ups.

## Finish within the requested scope

- For analysis, review, or planning, finish with the supported conclusion, recommendation, and remaining uncertainty. Implementation and commits require an implementation request.
- For authorized implementation, make one coherent change, inspect worker edits before accepting them, and run proportionate checks. Follow the repository's commit workflow.
- Include material contributions and unresolved risks in the final answer without repeating every worker report. Successful skill loading or worker completion establishes execution, not a speed or quality improvement.

## Evaluate changes to this skill

When tuning discovery or assessing effectiveness, use [the evaluation cases and comparison protocol](references/evaluation.md). This is a maintenance workflow, not an extra step for ordinary tasks.
