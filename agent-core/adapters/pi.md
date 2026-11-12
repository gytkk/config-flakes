# Pi-specific rules

These instructions extend the shared global agent rules with Pi-specific behavior.

## Runtime safety

- Pi is not sandboxed by default. Tools, extensions, and commands run with the
  current user's filesystem and network permissions.

## Execution discipline

- Treat user-reported failures as evidence. Investigate their cause rather than rerunning a failing command solely to confirm the report.
- When repository evidence and conventions settle a reversible implementation choice, choose the conservative option and proceed. Ask only at the shared decision and approval boundaries.
- Keep working while authorized, actionable work remains. A failed check is evidence to investigate, not by itself a blocker. If one part is blocked, finish independent in-scope work before reporting the missing prerequisite.

## Tool use

- Locate files before opening them. Reuse content already in context; read again when it was truncated, the file changed, or a failed edit indicates stale context.
- For symbol definitions and references, use the available LSP navigation tools. Check references before changing an exported symbol; prefer semantic rename and code-action previews for refactors. If the server or capability is unavailable, use targeted source inspection and report the limitation.
- Follow the active Pi tool schemas and extension guidance. Enable an on-demand provider when needed; do not assume another harness's tool names, URI schemes, or rendering features exist in Pi.
- For background shell tasks, use completion notifications and retained output instead of polling or rerunning the command to recover its result. Inspect live output only for a concrete progress or failure question. Background shell support does not authorize background subagents.

## Verification evidence

- Choose a check that exercises the changed behavior, not just the edited text. For UI changes, inspect the affected web, TUI, or CLI surface through the available runtime or project tools. If it cannot run, use the narrowest available fallback and state what remains unverified.
- Reuse tests that cover the affected path. Add a regression test for an uncovered bug; for other changes, add durable tests for plausible consumer-visible failures rather than wording, source layout, or implementation wiring.
- Distinguish observed results from inference. A child report, successful edit, or clean LSP diagnostics does not by itself prove runtime behavior. Report the checks actually run and their limits; apply the shared stopping rule once completion conditions are met.

## Pi skills

- When authoring Pi skills, use `writing-for-agents` for instruction design and `pi-agent`'s skills reference for Pi format and discovery. Store canonical shared skills under `agent-core/skills/` and declare exposure in `agent-core/manifest.toml`.
- Express required capabilities in shared skills; keep Pi packages, settings, extensions, and optional UI metadata in `modules/pi/`.

## Subagent context

- Scope the task inline before delegating. Keep a small, bounded edit or a question already answerable from loaded context in the parent; delegate only a distinct slice whose result can change the implementation or decision, or when the user explicitly requests it.

- Use one foreground child at a time in the current rollout. Background runs,
  workflow scripts, and delegated worktrees require a separate policy change.
- Before delegation, check that the role's required tools have providers loaded
  in the foreground child. Tool names alone do not load extensions. Report a
  missing capability instead of changing execution mode after a failed launch.
- For an authorized delegation, read `references/contracts.md` in the discovered `parallel-research-merge` skill directory and use the relevant contract branch. Reading this reference for an implementation worker does not select the parallel research workflow or authorize delegation. If the reference is unavailable, report the missing resource rather than guessing its contents.
- Default to fresh context. Include the task contract and relevant operating constraints in the child request: fresh children do not automatically inherit the parent's global instructions, operating addendum, or skill catalog. Use forked context only when the parent transcript is essential and cannot be summarized safely.
- When a child persists a substantial output artifact, prefer `file-only`
  output and read only the needed portions. Use inline output for short results
  or when no output path is available.
