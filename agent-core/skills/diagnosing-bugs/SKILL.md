---
name: diagnosing-bugs
description: Diagnose reported failures, incorrect behavior, intermittent bugs, and performance regressions using reproducible checks and evidence-driven experiments. Use for debugging or root-cause investigation, including diagnosis-only requests; not for general code review or speculative optimization.
---

# Diagnosing bugs

Connect the user's symptom to a cause through a check that can distinguish failure from success. Scale the investigation to the uncertainty: a clear failure may need one experiment, while an intermittent regression may need several. Diagnosis-only requests end with findings and verification limits; apply a fix when the user requested one.

## Establish the symptom and scope

Identify the trigger, expected behavior, actual behavior, affected environment and version, and any known working comparison. Read relevant code, tests, recent changes, and repository context such as `CONTEXT.md`, `GLOSSARY.md`, and nearby ADRs to locate the failing path. Ask only for missing evidence that changes the next step.

Separate observations from interpretations and distinguish the immediate failure mechanism from the conditions that produced it. A trace and matching code may explain the failing operation while leaving the origin of its inputs unknown. Recent changes suggest candidate causes. Code inspection and provisional hypotheses can guide reproduction before a runnable check exists.

Keep credentials in environment variables and redact secrets before displaying or saving diagnostic output. Captured requests may contain credentials or personal data; retain only the fields needed for diagnosis. Replay against an isolated fixture or authorized test environment, with external writes disabled or stubbed. Stress tests and production instrumentation require an appropriate environment and authorization.

## Build and confirm a feedback loop

Reuse the narrowest existing check that exercises the user's symptom. Otherwise choose a test, CLI or HTTP invocation, browser script, trace replay, or small harness at the relevant boundary. Compare versions or configurations when a known working state exists. If a human must perform a step, adapt the [human feedback template](scripts/hitl-loop.template.sh) or record equally explicit steps and observable results.

A usable loop records:

- The command or procedure, inputs, environment, and expected result.
- The observed result showing the reported symptom on the affected code path.
- A verdict that distinguishes the symptom from setup failures, missing dependencies, and unrelated errors. Mark those runs inconclusive rather than counting them as reproductions or successes.

Run the loop before changing the implementation and retain a concise baseline. Prefer seconds and repeatable inputs, but preserve the real failure when a slower integration check is necessary. A loop that exits successfully is useful only if its assertions would detect the reported symptom.

### Intermittent failures

Record failures out of total attempts, the time or attempt budget, and relevant seeds, concurrency, and timing. Define the counting unit; with retries, distinguish individual attempts from final request outcomes. Raise the reproduction rate with bounded repetitions or controlled scheduling when useful. Keep the original trigger as a separate check if stress or instrumentation changes execution. Rare failures remain evidence; zero failures in a short run do not prove a fix.

### Performance regressions

Define the user-visible metric and comparable workload before optimizing. Compare affected and known working states under equivalent data, configuration, resources, and warm-up conditions. Use repeated measurements and report variability; separate cold and warm runs when relevant. Profile or inspect query plans to locate the cost, then test a causal change. Use bisection when a reproducible signal and suitable version range make it informative.

### When reproduction is unavailable

Continue with available logs, traces, configuration, and code inspection. Label conclusions as confirmed observations or unverified hypotheses and state what would distinguish the remaining explanations. Do not claim a reproduced or verified fix from static evidence alone.

When further local work cannot reduce uncertainty, summarize the attempts and request the smallest missing artifact, access, or user-run check. Avoid repeating experiments without new information. A diagnosis with explicit limits is a valid outcome when execution is unavailable.

## Minimize and test explanations

Reduce inputs, steps, or dependencies one at a time, rerunning the check after each reduction. Retain the original scenario. Stop when the remaining case exposes the relevant mechanism and further reduction would cost more than it helps or remove the real interaction.

Rank plausible explanations using the evidence. State each hypothesis's prediction and the observation that would falsify it. Consider alternatives where evidence is ambiguous; do not invent a fixed number of hypotheses when the mechanism is already clear. For a longer investigation, share the leading hypotheses and next experiment without waiting for approval already granted by the task.

Choose the smallest probe that distinguishes the leading explanations. Change one causal variable at a time and record the prediction, result, and resulting decision. Use a debugger, targeted logs, a profiler, or a configuration comparison according to the failure. Tag temporary instrumentation with a unique prefix so `rg` can locate it later.

Keep track of rejected hypotheses and inconclusive experiments. Revisit an explanation only when new evidence warrants it. If evidence contradicts the current theory, revise the theory before stacking more fixes. Confirm that the proposed cause explains the original symptom; a nearby error disappearing is insufficient.

## Fix and verify within scope

For diagnosis-only requests, report the causal evidence, remaining uncertainty, and smallest proposed correction supported by the evidence without changing the implementation. If intended behavior is unknown, identify the missing requirement instead of inventing a correction.

When a fix is requested, make the smallest change supported by the evidence. Prefer a regression test at a boundary that exercises the actual interaction, such as multiple callers or the full event sequence. Reuse the confirmed reproduction when it already supplies that coverage. If automated coverage is impractical, retain a reproducible verification procedure and explain its limits; this alone does not establish an architectural defect or justify a refactor.

1. Confirm that the regression check fails for the reported reason on the unfixed implementation. A synthetic mutation checks the harness, but does not establish that the original bug was reproduced.
2. Apply the correction and run the same check without weakening the expected behavior.
3. Recheck the original scenario and relevant adjacent behavior. For intermittent or performance bugs, compare results under the baseline conditions and report the attempt counts or measurement variability.
4. Run the narrow project checks relevant to the change. Separate environment blockers from behavior failures.

If reproduction remains unavailable, describe any correction as unverified and give the exact follow-up check. Do not report the bug as resolved merely because unrelated tests pass.

## Finish

Remove only temporary instrumentation and artifacts created for this investigation, preserving user work and useful regression coverage. Use `rg` with the instrumentation prefix and inspect the final diff.

Report the cause and supporting evidence, what changed if a fix was requested, checks and outcomes, and any verification gaps or remaining blockers. Distinguish a mitigation that reduces symptoms from a correction of the cause. If the repository workflow calls for a commit or PR, record the causal explanation there.
