# Discovery and effectiveness evaluation

Evaluate discovery separately from behavior after loading. Record expected routing before running a case; give the evaluator only the request and raw artifacts, not the expected answer. Use read-only fixtures or an isolated temporary directory. Runtime launch commands, trace parsing, and credentials belong to the runtime integration.

## Discovery cases

| Request and available context | Expected routing and reason |
| --- | --- |
| "Use parallel research to compare the adapter configuration and actual session usage. Report findings only." Both sources are available and independent. | Load; one worker can inspect usage while the main agent inspects configuration. Finish with analysis. |
| "The old client fails after our API migration. Check the server's changed response contract and the independently maintained client's expectations before recommending a fix." Both codebases are available. | Load without naming the skill; the two investigations can change the diagnosis. |
| "Brighten this comment color from #545454 to #676767." The setting and existing verification command are known. | Do not select; make the bounded edit and verify it directly. |
| "Remove this redundant autostart entry." The entry and removal behavior are already identified. | Do not select; multiple affected documentation files do not create research questions. |
| "Compare these migration options, but use one agent and do not edit files." | Honor the single-agent analysis boundary; do not auto-select a parallel workflow. |
| "Investigate the client impact after discovering which endpoint the failing job calls." No independent work is yet available. | Resolve the dependency in the main thread; reassess only when a separable question appears. |
| "Find the documented timeout default in the official SDK manual." One documentation question is available. | Select `research`, not this coordination workflow. |
| "Have a worker implement this agreed parser fix in the named file and run its regression test." The change and ownership are settled. | Read the implementation contract reference for the authorized delegation; do not load the parallel research workflow solely because a worker is used. |

An explicit invocation tests loading, not automatic discovery. Confirm that the skill body entered context; a tool request or announcement alone is insufficient. Record successful loads, failed loads, valid non-selections, and unobserved cases separately. Unavailable authentication or trace visibility is not a correct negative. Do not run live writes or billable tasks solely to test selection.

## Behavior cases

Use realistic raw artifacts to check these outcomes independently of wording:

- An analysis-only request ends with a supported recommendation and no edits or commits.
- One independent question uses one worker while the main agent makes useful progress. An easy local edit proceeds directly.
- Conflicting worker findings are resolved against source versions or a discriminating check; unsupported agreement does not count as verification.
- A worker that reaches its investigation bound returns uncertainty. The main agent completes or cancels all outstanding units and preserves partial edits.
- The contribution note identifies which evidence changed the decision or verification, including a truthful no-additional-value result when appropriate.

## Task contract cases

These cases cover the shared task-completion rules and both branches of the contract reference. Prepare disposable raw fixtures and record expected outcomes before an authorized runtime evaluation. A text review or renderer test does not establish model behavior.

| Request and fixture | Expected behavior |
| --- | --- |
| "Explain why the parser rejects an empty field; do not fix it." Parser and failing input are provided. | Inspect the relevant code and report the supported cause and uncertainty; no edit or commit. |
| "Fix the empty-field case in the parser; preserve other parsing behavior." The regression test and existing conventions are provided to a fresh worker, without the parent transcript. | Use the supplied facts, owned paths, preservation constraints, and required checks to implement and verify the fix; return changed files and actual results. |
| The same worker believes a new dependency or public API change is necessary. | Report the evidence and required decision before making that change; the contract does not grant approval. |
| The regression passes, but the fixture's project instructions also require a parser compatibility check. | Run the required check before completing; the stopping rule does not skip project requirements. |
| All required checks pass and no concrete risk remains. | Finish without repeating the same checks or expanding into unrelated cleanup. |
| A required check cannot start because its runtime is missing. | Report the command and environment blocker as unverified; do not claim the behavior passed or install a dependency without approval. |
| A simple comment correction has a known location and check; history and runtime state are available but irrelevant. | Inspect applicable instructions and the affected context; avoid an unrelated history or runtime audit and do not delegate. |
| Research is requested with independent sources, but the runtime permits only one foreground child. | Respect the runtime limit; investigate sequentially where concurrency is unavailable and report the execution limit without changing policy. |

The existing discovery and behavior cases remain applicable. The implementation-reference case tests a distinct path, not a broader discovery scope for this skill. When only static review is performed, mark runtime discovery and behavior as unobserved rather than counting the expected outcomes as passes.

## Compare effectiveness

For an authorized effectiveness experiment, compare these conditions within each runtime:

1. Single agent with delegation disabled.
2. Native delegation without this skill or instructions requiring it.
3. Delegation using this skill.

Hold the model, effort, task inputs, tool access, worker limits, and acceptance criteria constant where applicable. Use the same tasks across conditions with fresh sessions; vary run order and repeat cases before treating differences as stable. Keep unrelated operating and safety rules unchanged. Test a skill revision against the previous revision under the same third condition.

Record correctness against predeclared acceptance criteria, unsupported findings, rework, duplicated investigation, evidence adopted, elapsed time to verified completion, and total parent-plus-worker tokens when the runtime exposes them. Mark unavailable metrics as missing. Count successful work separately from setup failures and incomplete runs. Evaluate outputs without revealing the condition when practical.

Report per-task results and limitations before aggregating. Loading frequency, worker count, or unmatched historical sessions cannot establish a causal improvement. Keep a revision only when its demonstrated benefit justifies its overhead for the intended tasks; do not generalize a small pilot to all work.
