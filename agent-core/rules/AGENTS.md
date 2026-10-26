# Global agent rules

These rules apply across agent harnesses.
Harness-specific instructions may adapt non-safety defaults to the deployed model and tools, but must not weaken the operating invariants.
Project instructions closer to the working directory override global defaults within their scope, but must not weaken the operating invariants.

## Skill discovery

- Before substantive work, check the available skills for a matching workflow.
- Use the best-fit skill when one exists and follow its instructions instead of improvising a duplicate workflow.
- If no skill fits, proceed without loading unrelated skills.

## Investigation and decisions

- Before changing files or asking questions, inspect applicable instructions and safely discoverable context: relevant code, tests, documentation, history, configuration, and useful read-only runtime state.
- When present, treat `CONTEXT.md`, `CONTEXT-MAP.md`, and relevant architecture decision records as repository-local sources of truth.
- Discover issue tracker conventions from maintained repository documentation and git remotes. Ask only when read-only discovery cannot identify the required workflow.
- Ask only about non-discoverable decisions that materially affect scope, rollout, UX, ownership, or architecture.
- When genuine alternatives exist, offer two or three mutually exclusive, substantive choices with a recommendation.
- Surface blockers, risky assumptions, and dependency issues early.

## Execution and planning

- For complex or ambiguous work, make a short, verifiable plan. Obtain approval before implementation only when work is high-risk or crosses an explicit approval boundary.
- Reconcile each planned item as done, blocked, or cancelled before finishing.
- Avoid repeated reading or editing without progress. If blocked, stop with a concise diagnosis and a targeted question.
- Communicate at meaningful milestones or when blocked. Avoid noisy updates for routine tool calls.

## Scope discipline

- Make the smallest change that fully satisfies the request.
  Keep every change directly traceable to the requested outcome.
- Do not add unrelated features, refactors, abstractions, or compatibility layers.
  Follow existing project patterns and remove only code made obsolete by the requested change.
- If the implementation grows beyond what the problem requires, stop and simplify it.

## Git

- Unless the user asks otherwise, commit each completed and validated change using only files changed for the current task.
  Follow the repository's commit style, falling back to a small imperative Conventional Commit when none is established.
- Do not push unless the user explicitly requests it.

## Repository and tool use

- Use `rg` (ripgrep), never `grep`, including in shell commands, scripts, and Nix expressions.
- Decide which files and searches are needed before calling tools, and batch independent reads where practical.
- Set the working directory explicitly for shell commands and avoid `cd` unless necessary.

## Editing safety

- Assume the worktree may already contain user changes.
- Ignore unrelated modified files.
  If a required file is already modified, understand and preserve those edits rather than overwriting them.
- Do not use destructive commands such as `git reset --hard` or `git checkout --` unless explicitly requested.

## Implementation and checks

- Handle errors explicitly and include enough context to diagnose failures. Do not silently ignore errors.
- For implementation tasks, use task-specific acceptance criteria and project-provided checks.
- Run the narrowest relevant tests, linters, and formatters first, then broaden checks according to risk.

## Code review

When reviewing changes:

- Treat the task as read-only unless the user asks for fixes. Inspect the task intent, applicable instructions, baseline, complete diff, tests, and affected call sites or dependencies before judging the change.
- Report concrete findings first, ordered by severity. For each finding, cite a precise path and line or range, explain the impact, provide evidence, and suggest the smallest effective remediation. Do not report stylistic preferences as defects.
- Distinguish confirmed defects from hypotheses. Run narrow checks only when they materially strengthen a finding, and report the exact command and outcome.
- If there are no findings, state that explicitly. Then report unresolved assumptions, verification gaps, and residual risks.

## Tool-specific workflows

- When opening a page for the user, including local previews and authentication links, use the system default browser: `open <URL>` on macOS, `xdg-open <URL>` on Linux, or the Windows default URL handler from WSL. Let the operating system select the browser and profile.
- For browser automation and testing, use `agent-browser` in headless mode by default. Use a visible browser window only when the user requests it or the task requires visible interaction. Keep page display requests on the system URL opener.
- If the system URL opener or `agent-browser` fails or is unavailable, report the error and ask before switching to another browser or automation tool.
- For Python, use `uv`. Run commands through `uv run`, not `python`, `python3`, or bare `pip`.
  If dependencies are missing, inspect project instructions and dependency metadata before retrying.
  Prefer `uv sync --locked` when a lockfile exists.
- For Notion, use the `ntn` CLI (`ntn pages`, `ntn datasources`, `ntn api`, and related commands).
  Do not use, add, or depend on a Notion MCP server.
- For Datadog, use the `pup` CLI.
  Start queries with narrow time ranges and small limits, and specify `--from` for time-series queries.
  When writing a script, runbook, or command that a user or CI will run outside the agent session, pass `--no-agent` so `pup` returns the same unwrapped output they will receive.
- These tool preferences do not authorize operations with external side effects.
  The approval requirements in the operating invariants still apply.

## Security

- Never expose or commit secrets, credentials, API keys, or generated authentication material.
  Use environment variables or the project's secret-management mechanism.

## Documentation

- Update existing maintained documentation when user-facing behavior changes.
  Do not create temporary or one-off documentation files unless requested.
