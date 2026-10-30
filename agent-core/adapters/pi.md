# Pi-specific rules

These instructions extend the shared global agent rules with Pi-specific
behavior.

## Skill routing

- Use `parallel-research-merge` when independent evidence can change a decision
  and the main agent has useful concurrent work. Routine delegation does not
  require loading a research workflow.
- Use parallel workers only when the session exposes an executable worker or
  subagent tool. Otherwise, run the same scopes sequentially.

## Repository tools

- Use `read` to inspect files and `edit` for precise manual changes.
- Formatting commands and generated outputs do not require `edit`.

## Runtime safety

- Pi is not sandboxed by default. Tools, extensions, and commands run with the
  current user's filesystem and network permissions.

## Pi skills

- Follow Pi's Agent Skills format: a focused `SKILL.md` with valid `name` and
  `description` frontmatter.
- Store the canonical shared skill under `agent-core/skills/` and declare its
  runtime exposure in `agent-core/manifest.toml`.
- Express required capabilities in the shared skill. Keep Pi packages,
  settings, extensions, and optional UI metadata in `config/modules/pi/`.
- Keep skills focused, trim unused files, and align them with repository
  conventions.

## Subagent context

- Default delegated runs to fresh context and pass a compact task contract with
  the required files, decisions, constraints, and validation.
- Use forked context only when the parent transcript is essential and cannot be
  summarized safely.
- When a child persists a substantial output artifact, prefer `file-only`
  output and read only the needed portions. Use inline output for short results
  or when no output path is available.
