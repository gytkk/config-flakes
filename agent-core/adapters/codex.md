# Codex-specific rules

These instructions extend the shared global agent rules with Codex-specific
behavior.

## Exploration and tool use

- Use sequential reads only when the next file cannot be identified until the
  prior result is known.
- Use `apply_patch` for manual file edits. Formatting commands and generated
  outputs do not require `apply_patch`.

## Codex skills

- When creating or updating repository-managed skills, use Codex's built-in
  `$skill-creator` as a scaffold when it is available.
- Store the canonical shared skill under `agent-core/skills/` and declare its
  runtime exposure in `agent-core/manifest.toml`.
- Express required capabilities in the shared skill. Keep Codex packages,
  settings, plugins, and optional UI metadata in `config/modules/codex/`.
- Trim unused scaffold files, keep the skill focused, and align it with
  repository conventions before committing.
- Use `parallel-research-merge` when independent evidence can change a decision
  and the main agent has useful concurrent work. Routine delegation does not
  require loading a research workflow.

## Model routing

- Use `gpt-6.1-sol` as the main/default agent for general work. Keep it responsible for coordination and final decisions when delegating difficult work. Reserve `gpt-6-astra` for subagents handling exceptionally complex tasks, such as architecture design with interacting constraints, deep debugging across multiple subsystems, or unresolved correctness questions that require deeper reasoning than Sol can provide.
- Apply the shared delegation criteria before dispatching. The main agent owns user interaction, requirements, decisions, integration, and final validation; workers handle the bounded units that pass those criteria.
- Keep short answers and one-step low-risk edits in the main agent. Spawn a worker only for a concrete unit that can run alongside useful main-agent work; keep dependent work sequential and give writing workers disjoint file ownership.
- Use native `spawn_agent` calls with explicit `model` and `reasoning_effort` values selected for the unit in the table below. Treat the table's effort as a starting point, not a requirement to use every model on every task.

| Model | Delegated work | Starting effort |
| --- | --- | --- |
| `gpt-6-luna` | Narrow, clear, repeatable tasks such as locating symbols, running a known check, or applying a mechanical edit | `low` |
| `gpt-6.1-sol` | Normal implementation, testing, research synthesis, and multi-step analysis with a defined scope | `medium` |
| `gpt-6-astra` | Subagents only for exceptionally complex tasks, architecture design, or deep debugging that meets the criteria above | `high` |

- Use `low` for direct lookups and deterministic checks, `medium` for ordinary reasoning, and `high` for tracing complex logic, checking assumptions, or analyzing edge cases. Reserve `xhigh` or higher supported efforts for especially difficult unresolved reasoning. A higher effort does not replace selecting a model suited to the task's ambiguity and failure cost.
- When a worker reports unresolved assumptions, conflicting evidence, or a failed validation it cannot explain, use `gpt-6.1-sol` for further investigation and ordinary design or correctness questions. Escalate to a `gpt-6-astra` subagent only when the remaining work meets the exceptional complexity criteria above. Pass the evidence and failed approaches to the next worker so it can continue the investigation.
- Set `fork_turns = "none"` explicitly by default. Include the relevant user requirements, constraints, owned paths, and acceptance criteria in the worker task. If the task needs parent conversation, choose the smallest sufficient positive turn count; use `fork_turns = "all"` only when the full history is required and state why in the worker task. Model or effort overrides require `"none"` or a bounded positive turn count; a full-history fork inherits the parent's model and effort.
- Inspect worker evidence and changed files before integration. Resolve conflicting results and run the relevant checks before reporting completion. If a requested model or effort is unavailable, select a supported alternative suited to the unit and report the fallback.

## Sandbox awareness

- Codex runs with `sandbox_mode = "danger-full-access"` in this setup.
- Exercise caution because commands have full filesystem and network access.
- Do not use destructive commands unless explicitly approved.

## Exec mode

- Assume non-interactive execution by default, for example `codex exec ...`.
- Make steps reproducible and deterministic.
- Prefer explicit command flags and stable output formats.
- Validate each major step with command output, file diffs, or checks.

## Structured output

- When `--output-schema` is provided, return strictly valid JSON that matches
  the schema.
- Do not add Markdown or prose outside the required structured output.
- Keep fields complete, accurate, and machine-readable.
