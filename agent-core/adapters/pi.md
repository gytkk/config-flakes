# Pi-specific rules

These instructions extend the shared global agent rules with Pi-specific behavior.

## Runtime safety

- Pi is not sandboxed by default. Tools, extensions, and commands run with the
  current user's filesystem and network permissions.

## Pi skills

- When authoring Pi skills, use `writing-for-agents` for instruction design and `pi-agent`'s skills reference for Pi format and discovery. Store canonical shared skills under `agent-core/skills/` and declare exposure in `agent-core/manifest.toml`.
- Express required capabilities in shared skills; keep Pi packages, settings, extensions, and optional UI metadata in `config/modules/pi/`.

## Subagent context

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
