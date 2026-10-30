# Claude Code-specific rules

These instructions extend the shared global agent rules with Claude Code-specific
behavior.

## Response language

- When a user-facing message contains two or more paragraphs before translation, provide the English original first, followed by a complete Korean translation in the same message. Preserve the original meaning, uncertainty, and technical identifiers in the translation.

## GitHub PR synchronization

- Treat `finish it`, `wrap this up`, `handle the rest`, PR preparation, test
  success, or local commits as insufficient permission to push. Push only when
  the current conversation explicitly requests it.
- Immediately after a successful push, check the current branch for an open PR
  with `gh pr view --json number,title,body,state`.
- If a PR exists, determine whether the pushed change made its title or body
  stale or inaccurate. Scope changes, invalidated test plan items, broken file
  references, and misleading summaries all count as stale.
- When stale, update the PR in the same turn. Preserve sections that remain
  accurate and rewrite only the parts that diverged.
- When the existing title and body still describe the branch correctly, leave
  the PR unchanged and report that decision in one line.
- This synchronization is part of an authorized push and does not require
  separate confirmation. If no PR exists, skip it silently.

## Planning interface

- For complex changes, use `EnterPlanMode` to design the approach and
  `ExitPlanMode` to submit it for approval.
- If plan mode tools are unavailable, present the plan as text and request
  approval.
- Revise the plan when the user provides feedback.

## Worktree workflow

- Work on the current branch unless the user explicitly requests a worktree.
- Project instructions may ban worktrees or impose stricter rules.
- Prefer the normal permission flow or `acceptEdits` in repository-local
  worktrees. Use `bypassPermissions` only in genuinely isolated environments
  such as containers or VMs.
- Before editing in a parallel-agent or worktree flow, verify the current path
  and branch. If the checkout is not the intended isolated worktree, stop before
  making changes and report any contamination clearly.

## Looping plugins

- When using Ralph Loop or another self-repeating plugin, set a bounded stop
  condition such as `--max-iterations`, an explicit `--completion-promise`, or
  both.
- Do not start an unbounded loop. If a loop becomes stuck, stop it and clear its
  state.

## Subagent delegation

- Use `parallel-research-merge` when independent evidence can change a decision
  and the main agent has useful concurrent work. Routine delegation does not
  require loading a research workflow.
- Delegate independent subtasks and continue useful work while they run. Prefer
  asynchronous or background subagents over spawn-and-block workflows.
- Intervene when a subagent goes off track or lacks relevant context.
- For long-running builds, use fresh-context verification on a reasonable
  cadence instead of relying only on self-critique.

## Prompts and skills authoring

- Store canonical repository-managed skills under `agent-core/skills/` and
  declare runtime exposure in `agent-core/manifest.toml`. Do not add a
  Claude-specific copy or plugin for an otherwise shared skill.
- Keep skills that require Claude commands, agents, or plugin metadata in the
  Claude marketplace instead of adding them to the shared catalog.
- When writing or updating prompts, skills, or agent instructions, state the
  goal and constraints rather than prescribing unnecessary step-by-step
  procedures.
- When migrating a prompt or skill to a newer model, prefer removing obsolete
  scaffolding and comparing results before adding new instructions.
- Explain why a delegated request matters, who it serves, and what its output
  enables.

## Discovery and reporting

- Audit progress claims against tool evidence from the current session.
- Report outcomes faithfully. State failed checks, skipped steps, and
  unverified claims explicitly.
- Do not use completion language while discovery, diagnosis, or required
  decisions remain incomplete.
- Keep exploratory responses explicitly in progress until the investigation is
  actually complete.
