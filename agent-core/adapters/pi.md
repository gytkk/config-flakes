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
- Use native `bash` for short non-interactive commands. For long builds/tests or input-driven CLIs, call `enable_interactive_shell` then `interactive_shell`. Dispatch builds/tests with `mode: "dispatch"`, `background: true`, and `handsFree: { autoExitOnQuiet: false }`; judge success from actual process exit, not silence. Use monitor mode only for a concrete output/state condition; keep semantic supervision and output selection off.
- Use shell completion notifications and retained output instead of polling or rerunning the command to recover its result. Inspect live output only for a concrete progress or failure question. Timeout, supervision cancellation, process exit, and process-tree termination are distinct: report only the state the package confirms. Running shell tasks survive `/reload`, not Pi shutdown or session replacement. Interactive-shell logs require explicit artifact registration; native bash retention keeps its provenance checks.
- Shell support grants no new delegation authority. Use the subagent contract below for authorized delegation, not interactive-shell `spawn`, `/spawn`, or shell-launched agent CLIs as an alternate launcher. These instructions are not a security sandbox.

## Verification evidence

- Choose a check that exercises the changed behavior, not just the edited text. For UI changes, inspect the affected web, TUI, or CLI surface through the available runtime or project tools. If it cannot run, use the narrowest available fallback and state what remains unverified.
- Reuse tests that cover the affected path. Add a regression test for an uncovered bug; for other changes, add durable tests for plausible consumer-visible failures rather than wording, source layout, or implementation wiring.
- Distinguish observed results from inference. A child report, successful edit, or clean LSP diagnostics does not by itself prove runtime behavior. Report the checks actually run and their limits; apply the shared stopping rule once completion conditions are met.

## Pi skills

- When authoring Pi skills, use `writing-for-agents` for instruction design and `pi-agent`'s skills reference for Pi format and discovery. Store canonical shared skills under `agent-core/skills/` and declare exposure in `agent-core/manifest.toml`.
- Express required capabilities in shared skills; keep Pi packages, settings, extensions, and optional UI metadata in `modules/pi/`.

## Subagent context

- Scope the task inline before delegating. Keep a small, bounded edit or a question already answerable from loaded context in the parent; delegate only a distinct slice whose result can change the implementation or decision, or when the user explicitly requests it.

- Within an authorized task, choose foreground or background execution and run independent children in parallel within the configured limits. Review and feedback follow-ups, including explicit native resume, may continue within that task's scope; expand requirements or authority only with approval. Workflow scripts, nested delegation, schedules, missions, and automatic resume on reload remain disabled.
- Give concurrent implementation workers separate worktrees and exclusive edit ownership. Use one writer per checkout, integrate shared-file changes sequentially, and preserve unrelated work. Creating a worktree does not authorize deleting it or applying configuration from it.
- Report and diagnose launch or runtime failures before proposing a bounded retry through the supported subagent tool. Follow the active tool's recovery rules; do not silently switch models, providers, or execution modes, or recreate a child through a shell command.
- Use native completion notifications for background children rather than polling or waiting in a loop. Account for every child as completed, blocked, failed, or cancelled before final integration; a notification alone does not verify its result.
- Before delegation, check that the role's required tools have providers loaded
  in the child. Tool names alone do not load extensions. Report a
  missing capability instead of changing execution mode after a failed launch.
- For an authorized delegation, read `references/contracts.md` in the discovered `parallel-research-merge` skill directory and use the relevant contract branch. Reading this reference for an implementation worker does not select the parallel research workflow or authorize delegation. If the reference is unavailable, report the missing resource rather than guessing its contents.
- Default to fresh context. Include the task contract and relevant operating constraints in the child request: fresh children do not automatically inherit the parent's global instructions, operating addendum, or skill catalog. Use forked context only when the parent transcript is essential and cannot be summarized safely.
- When a child persists a substantial output artifact, prefer `file-only`
  output and read only the needed portions. Use inline output for short results
  or when no output path is available.
