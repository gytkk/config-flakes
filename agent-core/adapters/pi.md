# Pi-specific rules

## Execution discipline

- Pi tools, extensions, and commands run with the user's filesystem and network permissions by default.
- Treat a reported failure as evidence. Use the next check to distinguish possible causes. If a prerequisite is blocked, finish independent authorized work before reporting what remains.

## Progress and final replies

- Use the task widget for routine progress. Interrupt for a material finding, a plan change, or a decision the user needs to make.
- Lead with the result. For routine changes, a sentence on the outcome and one on verification usually suffice. Include the evidence, trade-offs, or unresolved decision the user needs to act.

## Verification evidence

- For a substantive implementation, identify one plausible way it could pass existing checks but fail the user's requirement. Run the smallest check that distinguishes that failure from correct behavior. Reuse an existing test if it exercises the case; add a regression test for an uncovered bug.
- For UI changes, exercise the affected web, TUI, or CLI surface. When the intended check cannot run, use the closest available check and state what it leaves unverified. Distinguish observed behavior from inference, including when reporting a child's findings.
- For a design that would be costly to reverse, suggest an independent review before implementation if an objection could change the decision. Only with explicit user authorization, give one fresh-context, read-only reviewer the requirement, proposed design, and relevant code. Ask for the strongest concrete objection, if any. Verify it before changing the plan.

## Tool use

- Reuse current file reads. For symbol navigation, prefer LSP and check references before changing an exported symbol. If LSP is unavailable, inspect call sites and report the coverage gap. Probe changed paths with `lens_diagnostics` when cached results are absent or stale.
- Check Lens's effective project configuration before use. Keep automatic installation, formatting, fixes, tests, and startup scans off; report conflicting overrides before editing. Rename requires an explicit refactoring request, preview, and approval. Re-preview after intervening edits because apply requests fresh server edits. Keep AST paths bounded and replacements preview-only; use approved ordinary edits for replacements and listed code actions. Server commands and file moves require separate authorization.
- Use native `bash` for short commands and `interactive_shell` for long builds/tests or input-driven CLIs. Dispatch builds/tests with `mode: "dispatch"`, `background: true`, and `handsFree: { autoExitOnQuiet: false }`. Use monitor mode only for a concrete output or state condition; keep semantic supervision and output selection off.
- Use completion notifications and retained output rather than polling or rerunning. Inspect live output for a specific progress or failure question. Report process exit only when confirmed; silence, timeout, and supervision cancellation do not establish exit. Register interactive-shell logs explicitly when an artifact is needed.

## Pi skills

- When authoring Pi skills, use `writing-for-agents` for instruction design and the installed Pi package's skills documentation for format and discovery. In config-flakes, shared skill sources and selections belong in `agent-core/`; Pi packages, settings, extensions, and optional UI metadata belong in `modules/pi/`.

## Delegation and orchestration

For authorized delegation from a parent session, use the launcher the user requested. If its preflight fails, report the blocker instead of switching. Otherwise, when no launcher was requested, use the first available route below:

1. **jari chat tab:** When `JARI_TAB_ID` is nonempty, confirm that `jari orch list` succeeds for this tab and read `jari skill show orchestration`. Use `jari orch start` to create a new worker for each delegated task, including same-agent tasks. Follow the installed guide for dispatch and next-turn reports.
2. **Herdr terminal:** When `HC_TERMINAL_ID` is nonempty and `PI_SUBAGENT_CHILD` is not `1`, run `hc get "$HC_TERMINAL_ID"` and confirm the current terminal is live. Follow `hc --skill` and use `hc` as the sole launcher. Use `hc wait --children` only from the parent terminal; elsewhere, wait by worker ID.
3. **Other sessions:** Use Pi's native subagent tool.

The following worker constraints apply to every launcher:

- Keep bounded edits and questions answerable from loaded context in the parent. For authorized delegation, scope a distinct task inline and read `references/contracts.md` in the discovered `parallel-research-merge` skill directory. Use the applicable branch; if the reference is missing, report it. Reading the worker contract does not invoke a research workflow or authorize delegation.
- Check required tool providers before launch. Use fresh context unless the parent transcript is essential and cannot be summarized safely. Pass the task contract and relevant constraints: fresh children do not automatically inherit the parent's full instructions or skill catalog.
- Independent children may run in parallel within configured limits. Give concurrent writers separate worktrees and exclusive edit ownership; integrate shared-file changes sequentially. Worktree creation does not authorize deletion or applying its configuration.
- Use native completion notifications for background children. Reconcile each child's outcome and verify its evidence before integration. Prefer `file-only` for substantial saved artifacts and inline output for short results or when no output path is available.
- Report and diagnose launch or runtime failures before proposing a bounded retry under the tool's recovery rules. Do not silently switch models, providers, or execution mode. Native resume and feedback stay within the authorized task; wider scope requires approval. Workflow scripts, nested delegation, schedules, missions, and automatic resume on reload remain disabled.
