# Pi coding agent module

This module manages the global Pi coding-agent setup used by this repository.
It installs Pi and the NixOS MCP server, then exposes tracked configuration
under `~/.pi/agent/` through generated files and Home Manager out-of-store
symlinks.

## Managed resources

| Repository path | Runtime path | Purpose |
| --- | --- | --- |
| `modules/pi/files/settings.json` and `modules/pi/default.nix` | `~/.pi/agent/settings.json` | General settings and package selection merged into a device-local writable file |
| `files/web-search.json` | `~/.pi/web-search.json` | Web Access defaults |
| `files/lsp.json` | `~/.pi/agent/lsp.json` | LSP server routes and diagnostics settings |
| `files/mcp.json` | `~/.pi/agent/mcp.json` | Native MCP server configuration |
| `modules/pi/files/models.json` | `~/.pi/agent/models.json` | Context-window overrides for the configured OpenAI chat models |
| `agent-core/rules/` and `agent-core/adapters/pi.md` | `~/.pi/agent/AGENTS.md` | Generated shared and Pi-specific instructions |
| `agent-core/rules/OPERATING.md` | `~/.pi/agent/APPEND_SYSTEM.md` | Operating invariants added to Pi's system prompt |
| `files/extensions/` | `~/.pi/agent/extensions/` | Local Pi extensions |
| `modules/pi/agents/researcher.md`, `modules/pi/agents/reviewer.md`, and `modules/pi/agents/worker.md` | `~/.pi/agent/agents/` | Specialist prompts, tool contracts, and worker completion schema |
| `themes/exports/pi/one-half-light.json` | `~/.pi/agent/themes/one-half-light.json` | Generated One Half Light theme |
| `files/themes/claude-like.json` | `~/.pi/agent/themes/claude-like.json` | Alternate dark theme |
| Agent core skill sources | `~/.agents/skills/` | Codex/Pi shared pool, installed or updated by shared activation |

The module also installs:

- `pkgs.pi`
- `pkgs.mcp-nixos`

`settings.json` is a private, writable regular file on each machine, not a checkout symlink. Nix combines the general settings in `modules/pi/files/settings.json` with the `extensionPackages` selection in `modules/pi/default.nix` to generate the common settings. Home Manager merges these into the local file before linking the new generation; common top-level keys take precedence, while other local keys are preserved. Pi creates `deviceId` on first ChatGPT login and keeps it, `trackingId`, and `lastChangelogVersion` local. The sync rejects these fields in common settings. Existing symlink installations are migrated without writing their local state back to the checkout.

`/settings` and package commands modify only the local settings file. Changes to managed keys last until the next Home Manager application; edit `modules/pi/files/settings.json` for general defaults or `extensionPackages` in `modules/pi/default.nix` for package selection. Close Pi before applying Home Manager to avoid concurrent settings writes. Other mutable configuration still uses out-of-store symlinks. Generated instructions and common settings change only after applying Home Manager.

## Extension selection

`modules/pi/default.nix` declares the managed global extension selection in two places:

- `extensionPackages` lists Pi package sources, version pins, and resource filters in Pi's settings format. It supplies the generated `packages` setting; do not duplicate that key in `modules/pi/files/settings.json`.
- `localExtensions` maps installed extension names to checkout-backed or patched sources. Home Manager creates their links under `~/.pi/agent/extensions/`. The conditional Herdr state integration is declared beside this list.

Supporting files such as `modules/pi/files/extensions/subagent/config.json` remain separate from the extension entry points. `modules/pi/extensions/pi-lsp.nix` owns the LSP fetch and patch implementation; the selection lists show both its local entry point and the npm dependency package whose original extension is disabled. These lists do not enumerate Pi's built-in extensions or resources added by project settings or command-line arguments.

## Main settings

`files/settings.json` currently selects:

- `openai/gpt-6-astra` with `high` thinking
- the generated `one-half-light` theme
- a startup header without the loaded-resource listing
- a fixed `❯` prompt with space before the input and explicit one-column horizontal output padding (`outputPad: 1`)
- the hardware terminal cursor for IME positioning
- prompt-cache notices, with cache warming left at Pi's default `"streaming"` mode

Pi uses the default fullscreen mode and built-in keybindings: `Home` and `Ctrl+A` move to the input line's start, while `Ctrl+Home` moves to the top of the fullscreen conversation. The module does not install a keybindings file; its editor extension adds conditional background-task navigation described below.

### File links in Herdr

In fullscreen Pi running inside Herdr, Ctrl-clicking a local `file://` link, including the paths in `read` and `edit` tool headings, splits the Pi pane to the right and opens the file in Neovim. Keep Ctrl held until releasing the mouse button. The new pane takes focus, retains Pi's working directory, and closes when Neovim exits. Each Ctrl-click creates a new pane. Plain file-link clicks are consumed without launching Neovim or the platform's default editor, such as Zed. Dragging still selects text. Web links retain their normal click behavior; outside Herdr, Ctrl-clicked file links use the platform's default handler.

This behavior comes from the [local Pi package patch](../../packages/apps/pi/herdr-file-links.patch), not an extension or a Herdr Ctrl-click handler. It requires `nvim` on `PATH` and a working Herdr CLI. Invalid or remote file URLs are rejected. Launch failures are reported on Pi's stderr without falling back to another editor; if command submission fails after splitting, the new pane remains open for inspection.

Apply Home Manager and restart Pi to load the patched package; `/reload` alone does not replace Pi's code. Run `bun test packages/tests/pi-file-links.test.ts` from the repository root for routing, path quoting, and failure tests. To test mouse routing against a patched upstream source tree with its runtime dependencies available, run `PI_TUI_SOURCE=/path/to/patched/packages/tui/src bun test packages/tests/pi-file-clicks.test.ts`; this suite is skipped when `PI_TUI_SOURCE` is unset. After applying, confirm that a plain click on a file path in either a `read` or `edit` heading launches no editor, Ctrl-click opens the right-hand Neovim pane, and `:q` closes it. Regular mode delegates mouse input to the terminal and is not covered by this fullscreen click policy.

### Context windows and automatic compaction

`modules/pi/files/models.json` sets the configured OpenAI chat models, `gpt-6-astra`, `gpt-6.1-sol`, and `gpt-6-luna`, to a 1,000,000-token (1M) context window through `modelOverrides`. The overrides apply to parent and subagent sessions without changing authentication, endpoints, output limits, or other catalog metadata. They do not change the separate Luna classifier or the unused legacy `openai-codex` provider. This is Pi-side metadata for usage and compaction calculations, not a server capability change; the endpoint must accept the larger context.

Apply Home Manager to install the model configuration, then restart Pi. The prompt statusline shows the effective limit beside context usage, for example `11% / 1M`, and updates when the selected model changes. It uses the current usage metadata or the selected model before usage is known, rather than a hard-coded 1M label.

`compaction.modelOverrides` configures `openai/gpt-6-astra` to reserve 54,400 tokens and retain roughly 40,000 recent tokens without summarization. With the 1,000,000-token context window, automatic compaction starts above 945,600 tokens. Other models use Pi's default compaction budgets. The reserve remains an absolute token count.

### ChatGPT authentication

Run `/login openai` in Pi and choose `Sign in with ChatGPT`, then approve the browser login. Pi stores the OAuth credential under `openai` in `~/.pi/agent/auth.json`. Verify without printing credentials:

```bash
pi auth check --provider openai --json --no-refresh
```

The parent and delegated chat models use `openai`. The legacy `openai-codex` provider is not used. On existing installations, remove its saved credential through `/logout` and select `openai-codex`; its models then disappear from the authenticated model picker without affecting the `openai` login or historical sessions. In Pi 1.1, its GPT-6 Luna classifier is a separate model type and requires an API key; ChatGPT OAuth supports the chat model but not the Decisions API classifier. Use the existing `with-openai` wrapper in a separate process for API-key clients rather than replacing the subscription credential. See the [OpenAI API module](../openai-api/README.md).

`pkgs.pi` calls the official upstream Nix package definition from a pinned source release. [App package maintenance](../../packages/README.md#updates-and-ci) owns that source pin. Apply Home Manager to install the pinned Pi version and synchronize common settings.

`files/web-search.json` sets the default Web Access workflow to `none`, so
ordinary searches return directly without opening the curator. Requests that
need human source selection can still set `workflow: "summary-review"` or use
`/curator` explicitly.

The `extensionPackages` list in `modules/pi/default.nix` is intentionally version-pinned:

| Package | Version | Purpose |
| --- | --- | --- |
| `@juicesharp/rpiv-ask-user-question` | `2.15.0` | Structured user questions |
| `pi-lsp` | `0.1.7` | Lazy language-server diagnostics and navigation |
| `pi-bg-bash` | `0.5.1` | Background shell tasks, live output, and completion notifications |
| `pi-web-access` | `0.46.0` | Web search, source checks, and content fetching |
| `pi-subagents` | `0.84.0` | Foreground-first delegated Pi sessions |

Versioned npm packages are skipped by `pi update --extensions`. Update pins
through a reviewed repository change, and review package source and release
notes because Pi packages execute with the permissions of the Pi process.

`pi-subagents` loads only its extension; shared skills and the Pi adapter own delegation guidance instead of bundled skills and prompt templates. It uses packaged agents plus local `researcher`, `reviewer`, and `worker` definitions with explicit Luna/Sol/Astra routing. Delegation defaults to foreground and stores artifacts under the parent Pi session. Authorized tasks may use parallel or background children, isolated worktrees, and review/feedback follow-ups through explicit native resume. Nested delegation, workflow scripts, missions, schedules, automatic resume on reload, and the generic `delegate` agent remain disabled. Tool activation stays dynamic through `subagents_enable`.

Delegation defaults to `openai/gpt-6.1-sol`; `scout` and `researcher` use `openai/gpt-6-luna`, `planner` uses Sol, and `oracle` uses `openai/gpt-6-astra`. All three models are in the allowlist. The `reviewer` uses `high` thinking.

The packaged `planner` and `oracle` defaults are overridden to fresh context. The local researcher, reviewer, and worker also use fresh context. Use an explicit `context: "fork"` only when the child genuinely needs the parent transcript; otherwise pass a compact task contract. The researcher skips inherited project instructions because its role prompt and research task provide its operating context. Other agents retain repository instruction inheritance, but fresh children exclude the operator's global context and skills catalog unless explicitly configured otherwise. The child runtime also excludes the operator's `APPEND_SYSTEM.md`; a parent must pass relevant constraints in the task contract rather than assume its whole prompt is inherited. When a child persists a substantial artifact, prefer `outputMode: "file-only"` so the full result is not injected back into the parent context.

Only `worker` retains normal implementation tools. `scout`, `researcher`,
`context-builder`, and `reviewer` have no `edit` or `write` tool. Their `bash`
access is for inspection and verification and is not a security boundary; Pi
packages and child processes still run with the current user's permissions.
The Pi adapter permits execution choices within an already authorized task, not unsolicited delegation or broader authority. Concurrent implementation workers use separate worktrees with exclusive edit ownership; each checkout has one writer, and shared-file changes are integrated sequentially. Launch and runtime failures must be reported and diagnosed before a bounded retry is proposed under the tool's recovery rules. Model, provider, or execution-mode changes are not silent fallback paths. Background completion uses native notifications; the parent reconciles every child and verifies its evidence before integration.

`modules/pi/files/extensions/subagent/config.json` sets a run-wide concurrency limit of 8, with up to 16 tasks per parallel group and default parallel concurrency of 8. The session-wide cumulative spawn cap is disabled (`maxSubagentSpawnsPerSession: 0`); the package's separate per-run budget remains in effect. These are package execution limits, not a machine-wide process quota. `asyncByDefault: false` and `forceTopLevelAsync: false` preserve foreground defaults without prohibiting explicit background calls. `autoResume: false` prevents automatic revival on reload; explicit native resume remains available for authorized follow-ups. The managed `disabledFeatures: ["workflow-scripts"]` setting keeps native `tasks` and `chain` orchestration available and routes `/run` directly to a child compatible with Hub.

Apply Home Manager to refresh generated instructions, then restart Pi in a new session rather than resuming a session with the old policy. Model defaults and the allowlist are unchanged; pass an exact provider/model and thinking suffix when a task requires one. Agent Hub and the Herdr sidebar still have their documented foreground-oriented coverage; use package Fleet/status for other background runs. Enabling background execution does not extend those local UIs.

### Specialist contracts

`agents/researcher.md` loads the already pinned `pi-web-access` package through `subagentOnlyExtensions`. Foreground children do not load ambient extensions, and listing a tool name alone does not load its provider. The path assumes Pi's managed npm package location under `~/.pi/agent/npm/node_modules/`; review it when changing package installation layout. The role includes `web_enable`, the search/fetch tools, `source_check`, and native `contact_supervisor`. Web tools activate on demand. It has no shell or file-writing tools, does not request progress files or a default repository output, and returns evidence and gaps to the parent. An explicitly requested runtime artifact is still managed by pi-subagents.

`agents/reviewer.md` replaces the bundled reviewer prompt, which prohibits shell commands despite this module's inspection-oriented `bash` selection. The local role permits read-only Git inspection and focused local checks authorized by the task or repository instructions after their side effects are checked. It prohibits source/snapshot rewrites, auto-fixes, installation, staging, commits, branch changes, production access, and external writes. Unknown or unauthorized checks are reported to the parent rather than run. This is an instruction contract, not a shell sandbox; `acceptanceRole: "read-only"` does not restrict shell execution.

The local files own role prompts, context inheritance, tool selection, and child extension loading. `files/settings.json` owns model and thinking overrides. Avoid adding a settings override that silently replaces those role fields.

### Worker completion contract

`modules/pi/agents/worker.md` replaces the bundled worker. It retains the writer acceptance role, implementation tools, native supervisor escalation, fresh context and project instructions; model/thinking remain settings-owned. It has no default output, reads, or progress-file requirement. This is a profile for the existing `subagent` tool, not a second launcher.

After delegation is authorized, give each worker the task, allowed files, starting evidence, completion conditions, and approval boundary. A direct foreground call remains available:

```javascript
subagent({
  agent: "worker",
  task: "Implement the approved fix in the named files; run the authorized focused checks and report any unfinished required work.",
  async: false,
  options: { context: "fresh" }
});
```

Interactive `/run worker <bounded task>` uses the same profile. Apply Home Manager to install the definition, then restart Pi or `/reload`; inspect `/subagents worker prompt` and `/subagents worker details` to confirm the local definition. Project agents/settings can still shadow user profiles.

The frontmatter's inline JSON `outputSchema` uses pi-subagents 0.84.0's native `structured_output` tool. The final call supplies `value` with `contract: "pi-worker-completion-v1"`, `status: "completed" | "blocked" | "failed"`, `checks`, `incomplete`, and optional `summary`. Each check records an exact `command`, `result: "passed" | "failed" | "not-run"`, and observed `evidence` or the reason it was not run. There are at most 16 checks and 16 incomplete-work entries; commands, evidence, and incomplete entries are nonblank strings of at most 1,000 characters, and summary is at most 2,000. Unknown fields are rejected. `completed` requires an empty `incomplete` array. Required checks that could not run remain incomplete work; no-check tasks may use an empty checks array. The schema checks shape and consistency, not whether the worker omitted work or told the truth.

Missing or invalid structured output is not a successful contract result: the package requires a validated tool capture, not JSON-looking final prose. `completed` is the **worker's verdict**, not host-verified correctness. A package execution can finish normally with a `blocked` or `failed` worker verdict; parents must examine both. Reported commands are not independent proof that they ran or that the requested outcome is correct.

Writer acceptance remains available and distinct. Default foreground writer inference is `checked`, with no configured verification commands; inferred rejection is observable but does not necessarily fail execution. Explicit failed acceptance gates can fail execution. The package normally accepts `acceptanceReport` alongside `value`; `options.acceptance.report: "on"` requires it, while `"off"` selects the fenced acceptance-report path before the terminating tool call. When authorized, callers can use existing `options.acceptance.verify` or a plain `options.gate` for host-run commands. Neither the profile nor Hub adds commands, automatic verification, or independent reviewer launches. Host structural checks, host verification results (possibly memoized by the package), and independent review have different evidentiary limits.

Omit `options.outputSchema` to retain this contract. The package allows an explicit schema to replace it, and `false` to disable it; these are opt-outs, not schema extensions, and must not be used while claiming this completion contract. A JSON-output typed gate cannot coexist with an output schema; use the ordinary verification path when retaining the worker schema. Retained native resume uses the stored contract rather than adopting later profile edits. Hub may see only aggregate status/text for that child even when its schema was retained, so its verdict remains unknown until recognized structured evidence is available.

Run `bash modules/pi/tests/run.sh subagent-profiles.test.ts completion-contract.test.ts agent-hub.test.ts`. The completion test requires an already installed pi-subagents 0.84.0 (default managed npm location, or `PI_SUBAGENTS_PACKAGE_DIR`). It copies package source to a disposable directory, uses the real discovery parser, structured-output tool and validator, and exercises acceptance-report support and schema override precedence without a provider, child session, or verifier command. It also loads the managed execution config, exercises the package's concurrency scheduler and session spawn budget with local promises, and confirms that child-depth delegation remains blocked. These are structural/protocol and scheduler checks, not evidence of live parallel/background child execution, worktree isolation, actual worker compliance, live model schema compatibility, or implementation correctness.

### Prompt baseline and verification evidence

The 2026-11-02 baseline used Pi 1.4.0, pi-subagents 0.84.0, and pi-web-access 0.46.0. The installed agent definitions were resolved with the package's discovery code. Foreground child requests were captured after prompt hooks using the installed child-session factory and a local response stub, without sending the captured prompts to a provider. The parent instruction sources were inspected in the active session. Raw request captures remain outside the repository because they contain local paths and instructions.

| Role | Baseline prompt and context | Baseline issue | Managed contract |
| --- | --- | --- | --- |
| Astra parent, high | Pi base prompt, operating addendum, global/project instructions, skill catalog, active extension guidance | External research skill requires background execution and repository notes | At that baseline, the Pi adapter retained the foreground-only rollout; canonical research skill followed runtime policy and requested output |
| Sol worker, medium | Packaged replacement prompt, repository context; no global context, operating addendum, or skill catalog in fresh requests | Parent instruction inheritance must not be assumed | Local writer prompt and structured completion schema; parent supplies task-specific constraints |
| Sol reviewer, high | Packaged replacement prompt with repository context and shell prohibition; settings allow `bash` and legacy `intercom` | Role instructions conflict with shell selection | Local read-only validation prompt and native `contact_supervisor` |
| Luna researcher, low | Packaged replacement prompt with no inherited project context; no web extension loaded | Missing web providers and `source_check`; default output/progress expectations conflict with read-only research | Local prompt, explicit web provider, native supervisor, no default repository artifacts |

An isolated configuration with the managed profiles passed live foreground smoke checks on the same versions. Researcher called `web_enable`, `web_search`, and `fetch_content`, returned an official source, and distinguished current documentation from unverified version-specific evidence. Sol reviewer inspected a disposable Git diff, ran the explicitly authorized `node --test sum.test.mjs`, and reported the injected addition-to-subtraction defect and failed assertion. The fixture diff and index were unchanged. These checks establish loading and bounded role behavior, not prompt optimality or automatic skill discovery.

## Global prompts and skills

`agent-core/rules/OPERATING.md` is rendered as `APPEND_SYSTEM.md`, so it augments Pi's maintained system prompt instead of replacing it. The generated `AGENTS.md` combines shared methodology, prose guidance, and `agent-core/adapters/pi.md` without duplicating the operating invariants.

The shared [Agent skills activation](../agent-skills/README.md) installs or updates the manifest's Codex/Pi pool at `~/.agents/skills/` with pinned `skills@1.7.2` on each Home Manager switch, including when only Pi is enabled. It includes `devils-advocate`, `parallel-research-merge`, `pi-agent`, and `openai-decisions`. Before the first switch on an existing host, back up its skill files and CLI lock and resolve legacy ownership conflicts as described in [Agent core](../../agent-core/README.md#skill-설치와-복구). Verify discovery after switching and confirm that the old `~/.pi/agent/skills/` link is absent to avoid duplicates. A successful switch needs no separate installation command. The Pi module does not own skill copies or merge logic. The local `research` skill follows the runtime's delegation policy and creates repository notes only when requested or required by project instructions. Its source is `local` for the shared and Claude targets; activation backs up and replaces an existing `matt` installation.

### Task and delegation contracts

The shared task-completion rules in `agent-core/rules/AGENTS.md` ask the parent to derive the outcome, allowed scope, and observable completion conditions from the request and repository context. Simple requests do not require a user-filled form. Analysis ends with the requested findings; implementation includes relevant project checks. Once those conditions are met, further checks need changed code, a failure, an unresolved risk, or an outstanding project requirement.

Before authorized delegation, the Pi adapter points to [the shared worker contract](../../agent-core/skills/parallel-research-merge/references/contracts.md). Its common fields cover the task, starting evidence, allowed scope, preserved contracts, completion conditions, decision boundary, and actual results. The investigation branch adds source and result bounds; the implementation branch assigns local implementation choices to the worker and escalates changes to requirements, public APIs, dependencies, design, or ownership. The parent includes relevant operating constraints because fresh children do not inherit the whole parent prompt. Reading this reference for a Sol worker does not invoke the parallel research workflow.

The always-loaded Pi adapter retains runtime-specific constraints, while tool usage comes from Pi's tool guidance and skill format details are reached through the authoring skills. `research` covers primary-source investigation; `parallel-research-merge` covers independent investigations whose evidence needs reconciliation. Routine implementation delegation does not select that research workflow. The shared rules and skill references also affect Codex and Claude; the Pi reference-loading and child-context instructions are Pi-specific. Model and reasoning settings are independent of these contracts.

The Pi adapter adopts selected execution practices from OMP: evidence-driven failure investigation, focused reads, semantic navigation and refactoring when LSP is available, notification-driven background shell handling, and verification on the changed surface. These additions are Pi-only; shared rules, other runtimes, and the operating addendum remain unchanged. Small tasks stay in the parent, and blocked prerequisites do not stop independent authorized work. OMP-specific tools, internal URI schemes, rendering assumptions, personality, and parallel/background subagent policies are not imported. Pi's approval boundaries still apply; the current delegation policy above governs parallel and background children.

[Discovery and task-contract cases](../../agent-core/skills/parallel-research-merge/references/evaluation.md) define expected routing and stopping behavior. Renderer and source checks verify packaging and instruction placement, not automatic selection, model compliance, or an improvement in time or tokens. Apply Home Manager to refresh generated instructions and shared skill copies together, then start a fresh Pi session. Editing the canonical sources alone does not update an already loaded prompt.

## Local extensions

### Agent Hub

`/agent-hub` opens a Pi-native child selector using `modules/pi/files/extensions/agent-hub/`. Select a child to inspect its task, recorded model/status/result, or ask pi-subagents for current status and a transcript tail (up to 500 lines). The read-only viewer uses Pi's Text and ScrollView components; arrow keys and Page Up/Down scroll, and Escape returns to the selector. Viewing never resumes a child, changes the parent session, or injects its transcript into model context. Content is terminal-control sanitized and capped at 100,000 display characters.

Coverage is deliberately bounded: current-branch native foreground `subagent` and completed `/run` result records, live foreground tool updates with exact run IDs, and this Hub's own detached-resume receipts. Run ID plus the package's numeric child index identify each row; list position and opaque Fleet keys never become execution targets. It does not scan artifacts, enumerate arbitrary sessions, nested children, external runners, or other async jobs. Use `/subagents-fleet` for those views. Pi-subagents 0.84.0 slash progress does not expose an exact run ID, so an active `/run` appears only after its targetable result arrives. Missing model/result data remains unknown rather than being inferred.

Original parent sessions restore foreground rows from the active branch when their originating tool call or slash start is present. Pi-subagents redacts terminal tasks; Hub recovers caller-facing tasks by exact tool-call/request ID and numeric child index, including counted direct task lists. Chain templates and composite slash placeholders without unambiguous expansion stay explicitly unavailable rather than guessing a task. Forks/clones exclude copied history without an exact-owner, exact-child Hub observation. Observations persist the record projection through Pi's `appendEntry`, including partial progress, so newly observed fork rows remain restoreable without a terminal result. Branch navigation clears pending tool/slash correlations and invalidates dialogs and reads; late replies cannot create ownership on the new branch. Abandoned branches are not roster sources.

**Follow-up is a separate, explicit action.** Choose "Request follow-up", inspect package status, enter a nonempty message (up to 4,000 characters), and confirm "Start detached background follow-up?". This is Hub's user-confirmed route for a follow-up. The parent can also use native delegation tools for follow-ups within an authorized task; Hub confirmation does not authorize unrelated work or enable automatic resume, workflow scripts, or nested delegation. The extension exposes no model-facing tool or launch command. Recorded running, pending, detached, and stopped foreground rows have no follow-up action. Other rows are candidates, not a promise of resumability: pi-subagents authoritatively checks current ownership, stored context, live/stopped state, capability ceilings, and session leases. Multi-child runs always send the exact selected index.

RPC `resume` creates a **new detached background child from stored context**, not a parked/live-session reattachment. It retains the package-owned model/tools/budgets; the Hub offers no overrides, stop, or steer controls. Each accepted single-run receipt establishes the new exact run and index `0`, previous target, and lineage root. Composite/conflicting receipts fail closed. Completion changes the execution observation, not the retained row identity. Continue from the latest accepted row, never the old source. The package alone owns live sessions, process resources, transcript retention, leases, and eligibility; Hub has no idle/parked states, cache-retention TTL, or local session recreation.

Only accepted Hub resume targets receive automatic read-only refreshes. Startup/reload and package job/completion events coalesce targeted `status` reads; Hub consumes matching roots in the public versioned async snapshot and exact native completion notices, never an exhaustive async/nested roster. Inspection also refreshes these observations. Duplicate notices are harmless; late snapshots cannot undo native completion evidence, and stopped state is irreversible for the same exact run. A legitimate transition from paused to stopped remains possible. Queued/running/accepted rows remain inspectable but cannot request follow-up until terminal evidence is observed; stopped rows remain blocked. Native completion records and owner-bound observations restore after reload. Status is last-observed evidence, not a live-session handle: omitted/expired snapshot entries or a missing API do not prove completion or eligibility. Closing Hub, switching sessions, reloading, or timing out does not cancel dispatched work; package completion may wake the parent.

The selector labels execution separately from the worker verdict. The record viewer recognizes only the bounded versioned worker contract in terminal native `details.results[].structuredOutput`; it never scrapes prose, acceptance fences, or transcript text. Missing, malformed, unrecognized, or package-rejected structured output leaves the verdict unknown. A resume receipt clears the source child's verdict and acceptance evidence, and aggregate snapshots/completion notices do not invent a replacement verdict. The viewer separately projects the public acceptance ledger's status, evidence status, host structural checks, and host verification commands, never promoting child-reported commands into host evidence. Unknown or oversized ledger projections remain unknown (16 checks/commands, 1,000-character fields); display content remains sanitized and bounded. These fields are observations, not Hub approval or follow-up eligibility gates.

The integration uses only Pi 1.5.2 public extension APIs and pi-subagents 0.84.0's documented event-bus RPC (`ping`, `status`, `resume`). Owner and branch generations guard asynchronous reads, confirmations, and persistence. Each request has a unique correlation ID and is recorded before dispatch. An explicit package failure reply is recorded as rejected (not proof that every side effect rolled back); timeout, transport failure, or an ambiguous success stays unknown. Neither is automatically retried. A session-wide exact-owner dispatch guard prevents duplicate revival even after navigating to a branch predating the request, without adding abandoned rows to the roster. Replacement/fork cannot inherit that authority. Unknown requests cannot safely recover a new run ID through the published resume contract; Hub will not guess from Fleet or reuse the documented spawn-only alias. Inspect package status/Fleet for manual recovery. Even rejected targets remain guarded in Hub; this is conservative duplicate prevention, not durable reconciliation or exactly-once delivery.

Apply Home Manager manually to install the extension, then restart Pi or run `/reload`. Later checkout-backed edits load with `/reload`. Run `bash modules/pi/tests/run.sh agent-hub.test.ts` for package-shaped redaction/snapshot/completion fixtures, exact-index lineage, reload/fork/branch restoration, late-event and duplicate-dispatch races, read-only and explicit-resume flows, native-component rendering, and Node/jiti discovery. These tests do not launch a provider or child. A live user check should browse an existing completed foreground child, cancel a follow-up without execution, and only when desired explicitly confirm a small follow-up and verify the package's detached receipt and completion behavior.

### Herdr parent status

When both Pi and Herdr modules are enabled, Home Manager installs `herdr-agent-state.ts` from the pinned Herdr source into `~/.pi/agent/extensions/`. This official integration reports the parent TUI session's working, idle, and blocked states. `agent_start` marks it working; `agent_settled` marks it idle only after automatic retries, compaction, and queued continuations finish. Headless child sessions do not report parent state.

Apply Home Manager and run `/reload` in Pi, or start a new Pi session inside Herdr, to load the integration. Home Manager owns this file; update it through the Herdr package pin rather than `herdr integration install pi`. The display metadata extension below does not report lifecycle state.

### Herdr subagent sidebar

`files/extensions/herdr-subagents.ts` reports the current parent session model and displays foreground `pi-subagents` children beneath their parent Pi entry in Herdr's expanded Agents sidebar. The parent row shows the agent name and model, updating when the selected model changes. Each child uses two lines: its agent name and resolved model, followed by its current tool and path, latest output, or assigned task. Both the `subagent` tool and `/run` foreground execution are supported. The layout hides pi-subagents' background summary; this extension does not enumerate background children.

The extension publishes display metadata only from the parent interactive Pi session inside Herdr. Completed, failed, interrupted, and detached foreground children disappear from the list. Session shutdown clears its tokens; active metadata expires after 45 seconds without refresh if Pi crashes. At most seven children fit in Herdr's 16-line layout. Larger groups show the first six children and an overflow count. Clicking any child line focuses the parent pane.

The progress payload and slash events follow `pi-subagents` 0.84.0. Review this integration when updating that package. It does not import the package's internal modules or modify its execution behavior. Herdr failures produce a warning and leave Pi running.

Home Manager installs the extension. After applying the configuration, run `/reload` in Pi and `herdr server reload-config` for the [Herdr sidebar layout](../herdr/README.md#subagents). For a foreground smoke test, run `/run scout List the top-level files without changing anything` inside a Herdr Pi pane and verify that the child appears, updates, and disappears when finished. Run `bash modules/pi/tests/run.sh herdr-subagents.test.ts` for the isolated event and metadata tests.

### Herdr Desktop

The same extension reports foreground `subagent` and `/run` children to the [personal Desktop Agents mod](../herdr/README.md#desktop-subagents) using the parent core terminal's `pi.subagents` label. `HC_TERMINAL_ID` identifies the terminal; `HC_TERMINAL_SOCKET` takes precedence over `HC_SOCKET`. A valid Herdr TUI pane uses only pane metadata, never the enclosing terminal label. Headless sessions and `PI_SUBAGENT_CHILD=1` processes do not report.

Desktop rows show the agent name, resolved model when known, and a state-colored dot. Tool/path, recent output, or task text appears in the tooltip; tool arguments and reasoning are not read. Pending, running, completed, failed, and interrupted states follow the package's progress and per-child results. Settled rows remain for five seconds. Detached children disappear rather than being called complete; this foreground integration does not monitor background runs. Desktop has no seven-child limit, and clicking a row focuses its parent without changing the parent's lifecycle.

Snapshots refresh every five seconds and expire in the UI after fifteen seconds without an update. Session replacement gets a fresh identity; shutdown removes the label after queued writes finish. Core labels are persistent: activity excerpts can remain on disk after a crash even though stale rows are hidden. Remote reporters and the desktop need synchronized clocks.

Apply Home Manager on the reporter and desktop hosts, select the personal `tui` layout, then run `/reload` in Pi or restart it. Desktop mods hot-reload; `herdr server reload-config` is only for the TUI. Verify with `/run scout List the top-level files without changing anything`. Checks: `bash modules/pi/tests/run.sh herdr-subagents.test.ts` and `node --experimental-vm-modules --test modules/herdr-app/tests/omp-agents.test.mjs`.

### OpenAI fast mode

`modules/pi/files/extensions/codex-fast-mode.ts` provides:

- `/fast [on|off|status]`
- `service_tier: "priority"` for `openai` requests while enabled
- fast-mode status for the prompt's integrated statusline

Fast mode starts disabled in each new session; use `/fast on` to enable it. Toggle changes are stored in the session so resumed branches recover their previous state. The extension owns the priority-mode request field and publishes its display state through `codex-fast-mode`. The prompt UI consumes that status without importing the fast-mode implementation.

Subagent child processes are detected through `PI_SUBAGENT_CHILD=1` and do
not receive the priority service tier. `PI_SUBAGENT_PARENT_SESSION` is also set
in the parent UI session for permission forwarding, so it must not be used as
the child-process signal. Parent Pi sessions continue to use fast mode normally.

### Codex subscription usage

`modules/pi/files/extensions/codex-usage.ts` adds `/codex-usage` and shows the Codex weekly quota remaining and time until reset immediately to the left of the model in the prompt's top statusline. The compact format is `#######--- 79% (⏳ 3d 14h)`: a ten-cell remaining-quota bar, the remaining percentage, and the reset countdown in days and hours. The countdown uses 24-hour days and drops partial hours; less than one hour remaining appears as `0d 0h`. Each `#` represents a full ten percentage points remaining and appears in terminal-palette green in the statusline; unfilled `-` cells retain their default color. An unknown reset time appears as `(⏳ unknown)`. Narrow terminals truncate the usage text or omit it to preserve context and model information. Interactive parent sessions fetch usage at startup and every minute without blocking startup. `/codex-usage` refreshes immediately and shows the local reset date and time without a time-zone suffix, along with the source timestamp. Concurrent refreshes share one request, each request has a 30-second timeout, and shutdown or reload cancels pending work. Non-interactive sessions and subagent processes do not poll.

The extension runs `codexbar usage --provider codex --source oauth --json --no-credits` using the CodexBar CLI already installed by the common profile. It reads the weekly `usage.secondary` window, not session token totals or API credit balances. OAuth uses the Codex CLI account selected by CodexBar and `CODEX_HOME`, which can differ from Pi's ChatGPT login. Use the same account in both clients to track the subscription used by Pi. The extension does not read, copy, or manage authentication files; CodexBar owns usage retrieval and its authentication behavior. It does not fall back to browser cookie extraction.

Failed queries replace the displayed quota with an unavailable status; `/codex-usage` reports a bounded error without raw CLI output or account identifiers. Data older than ten minutes is marked stale. Missing reset times remain unknown, and an elapsed reset is marked as awaiting fresh usage rather than assuming a replenished quota. Usage stays in memory and is not added to the model context or session history.

Apply Home Manager and restart Pi to install the extension. To try it before applying Home Manager, start `pi --extension ./modules/pi/files/extensions/codex-usage.ts` from the repository root. If a fetch fails, check that `codexbar` is on `PATH` and that the Codex CLI account is authenticated; Pi's `/login openai` does not authenticate CodexBar.

### Prompt and cursor rendering

`modules/pi/files/extensions/prompt-ui/index.ts` owns the editor and the empty footer. It connects the input behavior in `modules/pi/files/extensions/prompt-ui/editor.ts` to the renderer in `modules/pi/files/extensions/prompt-ui/statusline.ts`. Only the directory entry point loads as an extension. Fast mode and usage retrieval remain separate extensions that publish through Pi's status API. The UI works without either provider. It shows `fast` beside the thinking level only when fast mode is enabled for an OpenAI parent session; otherwise it omits the label and its separator. The statusline shows working directory, Git branch, model, thinking level, fast-mode state, context usage, and extension statuses; it does not aggregate cumulative input/output tokens. The working directory and branch share a subtle theme-backed background with a rounded Powerline left cap and an arrow-shaped right cap, followed by the context meter. The caps use the same color as the segment background and require a font with Powerline glyphs, such as a Nerd Font. Other extension statuses, weekly quota, and the model appear on the right, with no trailing border after the model group. The thinking level uses terminal-palette purple; the enabled `fast` label retains its orange accent. Background tasks appear on the input row instead. The context meter occupies the remaining width, centers its percentage label, and scales its filled line across that full width. A padded vertical divider separates the meter from the right-hand status group. At narrower widths, the renderer shortens the path and quota, then omits secondary statuses and the location before shortening essential context and model information. A selected `/bg` item retains a visible marker and hardware cursor on the input row even in a very narrow pane.

Git file counts appear immediately after the branch, for example `main *12 +4 ?13`: `*` counts unstaged tracked paths (including unresolved conflicts), `+` counts staged paths, and `?` counts individual untracked files. A partially staged file contributes to both `*` and `+`; renames count once per category. Zero counts and clean repositories add no text. The prompt polls Git asynchronously every two seconds after each query finishes, including while idle, without refreshing the index. Queries have a five-second timeout and a 4 MiB output limit; failed or incomplete queries show `git!` instead of stale counts and report a warning. Shutdown, session replacement, and `/reload` cancel polling and pending queries. Non-TUI sessions do not poll. The counts share the branch segment and disappear with it when space is insufficient. Run `bash modules/pi/tests/run.sh prompt-git-status.test.ts prompt-statusline.test.ts prompt-ui.test.ts` for count parsing, polling lifecycle, and rendering checks.

The first visible input row starts with `╰─` followed by one space instead of a boxed input area. Built-in editor padding reserves three columns for the marker and gap, so neither is part of submitted text. Very narrow terminals reduce the padding to leave room for input. At three or four columns it uses only `╰`; below three columns it omits the marker. The editor keeps Pi's wrapping, history, autocomplete, and mouse positioning. Hidden input lines retain scroll hints. The bottom border is omitted when no lines are hidden below; autocomplete retains a blank separator so its mouse coordinates remain unchanged.

The extension removes Pi's software cursor markers while preserving the terminal cursor used for IME positioning. It is paired with `showHardwareCursor: true` in `modules/pi/files/settings.json`. Review it when updating Pi if cursor rendering or IME positioning changes. Apply Home Manager and restart Pi when migrating from the former hardware-cursor-only extension so the old entry point is removed and the prompt UI directory is installed together. After that migration, `/reload` picks up checkout-backed prompt UI edits. Check a narrow pane, multiline Korean input, autocomplete, and Down/Enter task selection after reloading.

### Terminal title on exit

Pi sets the terminal title to `π - <directory>` and does not clear it on exit, so terminal applications can keep showing it after returning to the shell. `files/extensions/terminal-title-reset.ts` sends an empty title when the TUI process exits after a quit. It writes from a process `exit` listener because Pi 1.5 rewrites the title after `session_shutdown`. Session reload, new, resume, and fork leave the title to Pi.

## On-demand tools

Fresh sessions use the packages' own `web_enable` and `subagents_enable` tools to activate the full web and delegation tools when needed. Package commands such as `/run` remain available. Native MCP uses Pi's `codemode` tool without declaring every server tool to the model. The module does not install a custom `enable_tools` tool or `/tool-profile` command.

When migrating an existing installation, apply Home Manager to remove its managed `~/.pi/agent/extensions/tool-profiles` symlink, then restart Pi. New sessions use the default activation state; resumed sessions may retain tools already activated in their transcript.

## Rounded tool and extension-message frames

`modules/pi/files/extensions/rounded-tools.ts` wraps each tool's call and text result in one rounded terminal border. It uses Pi's renderer resolver without replacing tool definitions or execution. Existing renderers retain syntax highlighting, edit previews, codemode nested-call status, streaming updates, and expansion controls. Like OMP's terminal output blocks, the renderer applies the tool background to each complete framed row, including the rounded corners, horizontal rules, and side borders. This removes the unpainted band between the content and outline. The fill remains character-cell based, not a pixel-clipped rounded shape. The outline uses the same background hue, shaded 15% toward black in light themes or white in dark themes for subtle contrast. Pending, successful, and failed calls retain their theme backgrounds. Tools with their own shell retain their inner layout and explicit background colors; uncovered cells use the tool-state background. Panes narrower than five columns omit the border.

Pi renders native image attachments below the text frame. Custom renderers that emit inline Kitty or iTerm2 graphics bypass the frame to preserve terminal image placement. Empty renderers stay hidden, and a failing renderer shows a warning with plain call or result text. Generic tools without renderers show their arguments and a collapsible text result.

The extension also frames every displayed custom message, including background Bash output/completion, subagent results and notices, web-search messages, and future custom message types. It wraps the existing renderer (or Pi's default Markdown renderer) rather than replacing the message body, so custom controls, links, expansion, and explicit inner colors remain intact. Assistant/user messages, user-entered shell commands, dialogs, and custom entries registered through `registerEntryRenderer` are separate surfaces and are not changed.

Both `lsp_diagnostics` calls and automatic `pi-lsp-diagnostics` notices use this frame. They collapse to two content rows: the tool name and path, then severity counts and an expansion hint. Server availability warnings retain their reason instead of being presented as a clean result. `Ctrl+O` (or the configured tool-expansion key) reveals the received diagnostic text; upstream omissions remain labeled as additional diagnostics. Narrow panes truncate the summary instead of wrapping it. Display compaction does not change tool results, stored notice data, or model context.

The local `packages/apps/pi/message-renderer-wrappers.patch` adds `pi.registerMessageRendererWrapper(wrapper)`. Its TUI-only callback receives `(component, message, options, theme)` and returns a component. Wrappers run after the original renderer or its default fallback, in extension load order and then registration order. Expansion, theme changes, and history restoration rebuild the wrapped component. A throwing wrapper shows a warning and preserves the preceding component; the hook does not alter session data, model context, or non-TUI output.

Apply Home Manager to install the patched Pi package and the updated `modules/pi/lsp-refactor.patch`, then restart Pi. `/reload` cannot load a package-level API change. Until the patched Pi is installed, this extension retains tool/LSP frames and shows an upgrade warning; other custom messages keep their prior appearance. The patch removes pi-lsp's original notice renderer so this extension is the sole renderer for that message type; `/reload` alone cannot replace the previously installed patch. Run `bash modules/pi/tests/run.sh rounded-tools.test.ts lsp-diagnostics-rendering.test.ts custom-message-frames.test.ts` to check the installed Pi renderers, compact diagnostics and expansion, mouse coordinates and file links, narrow panes, theme changes, image placement, and Node/jiti discovery. For a live check, expand a `read` result, inspect an `edit` diff and a codemode result, and confirm that image attachments remain below the frame.

## Background shell tasks

`pi-bg-bash` replaces Pi's model-facing `bash` tool, not the system Bash executable, login shell, or user-entered `!` commands. Ordinary calls still wait for exit; `background: true` returns a task ID immediately, and completion sends the agent the exit state and recent output. `Ctrl+Shift+B` moves running foreground tool calls to the background when the terminal supports modified keys. Elapsed time alone does not trigger that transition.

Use `/bg` to view task state, runtime, and commands. The first visible input row shows `/bg (N)` right-aligned only when the command is available and at least one task is running. The display uses unused space without reducing the editor's wrapping width or moving its mouse targets. It hides when it would overlap input text or the cursor, and while autocomplete is open. `N` is the number of running tasks reported by pi-bg-bash's status, excluding finished tasks. A zero, absent, or unrecognized count hides the item and clears its selection. The parser in `modules/pi/files/extensions/prompt-ui/editor.ts` isolates the dependency on pi-bg-bash's `bg: N` display format. When tasks are running, from an empty input with autocomplete closed, press Down to highlight `/bg (N)` in the terminal palette's purple, Enter to open `/bg`, or Up/Escape to return to the input. The editor publishes selection through the `bg-selection` status and highlights the right-aligned item without adding a row or duplicating the package's `bg: N` status. Typing or pasting leaves the selection and edits the input normally. Nonempty input retains normal cursor and history navigation. In `/bg`, press Enter for live output, `x` to stop a task after confirmation, and Escape to go back. The agent can use `bash_tasks`, `bash_output`, `bash_kill`, and `bash_watch` to inspect or manage tasks. Output watches are optional; completion notifications do not require them.

Tasks survive `/reload`, but quitting Pi or switching sessions stops them. Logs are private files under `$XDG_STATE_HOME/pi-bg/` (default `~/.local/state/pi-bg/`), with seven-day cleanup and a 100 MiB per-task limit. The package does not honor Pi's `shellPath` or `shellCommandPrefix`; neither is configured by this module. Do not combine it with another extension that replaces `bash` or registers `/bg` or `Ctrl+Shift+B`.

After installing the pinned package, run `/reload` or restart Pi. Verify with a disposable background command that prints a line, waits briefly, and exits; inspect it in `/bg` and confirm its completion notification. This shell-task extension does not enable background subagent delegation.

## LSP diagnostics

`pi-lsp` starts a configured language server only after a matching file is
written, edited, or passed to an LSP navigation tool. After a successful
built-in `edit` or `write`, diagnostics for that file are appended to the tool
result so the model can correct problems in the same turn. The package also
provides `lsp_diagnostics`, `lsp_hover`, `lsp_definition`, `lsp_references`,
and `lsp_symbols` tools.

`files/lsp.json` configures the following servers from `modules/lsp`, plus
`rust-analyzer` installed in the active rustup toolchain. The LSP module also installs Pyright for other consumers, but Pi's Python route uses only `ty`:

- `nixd`, `gopls`, and `typescript-language-server`
- the JSON, CSS, and HTML servers from `vscode-langservers-extracted`
- `lua-language-server`, `bash-language-server`, and `terraform-ls`
- `ty`, `yaml-language-server`, `marksman`, and `taplo`
- `rust-analyzer`

Server processes are scoped by configured project root markers and stop when
the Pi session shuts down. Generated output, dependency directories, and
language-specific build directories are excluded where applicable. Missing
binaries produce diagnostics warnings instead of causing Pi startup to fail.

LSP output is targeted feedback, not the authoritative project check. Continue
to run the repository's formatter, tests, type checker, or build before
finishing a change. `pi-lsp` is a third-party package that runs configured
binaries with the permissions of the Pi process, so update its version pin only
after reviewing the new source and release notes.

### Semantic rename and code actions

`lsp_rename`, `lsp_code_actions`, and `lsp_apply_edit` are available by default,
like the existing LSP navigation tools. No slash command or enable step is needed:
ask for a refactoring directly, review the returned diff, then authorize apply.
Tool definitions are always available to the model; they add no prompt snippets
or guidelines. Language servers still start only when needed. All positions use
**zero-based lines and UTF-16 character offsets**, matching the navigation tools.

- `lsp_rename({ path, line, character, newName })` uses the server's rename
  preparation when supported and returns a multi-file preview, not a mutation.
- `lsp_code_actions({ path, line, character, endLine?, endCharacter?, only? })`
  lists actions at a range. The end defaults to the start; `only` accepts LSP
  kinds such as `quickfix` or `source.organizeImports`.
- `lsp_code_actions({ actionId })` resolves a listed action when necessary and
  previews its edit.
- `lsp_apply_edit({ previewId })` applies exactly that preview. Tokens are
  in-memory, session-local, single-use, and cleared on session replacement,
  shutdown, or reload. Listing actions replaces the previous action list.
  Only the latest eight previews are retained.

Preview does not write files. Apply locks affected files using Pi's shared
mutation queue, checks file identity/content and available document versions,
then stages replacements preserving file modes. A stale file rejects the whole
edit before writing. Failed writes attempt guarded rollback without overwriting
subsequent user edits. External editors do not share these locks: there is still
a final check-to-rename race, and a process crash is not a cross-file transaction.
Successful edits synchronize already-open documents on the existing LSP clients.

Only UTF-16 text edits to existing UTF-8 files inside the selected server's
project root are supported. Invalid/overlapping ranges, symlink escapes,
unsupported server capabilities, and mixed resource operations are rejected.
Servers such as gopls can report version zero for unopened on-disk documents;
these use disk snapshot checks rather than a client document version.
Command-bearing actions are listed as unavailable: executing arbitrary server
commands cannot guarantee a side-effect-free preview, even when an action also
contains an edit. File creation, deletion, and file moves are not supported.
Use the editor for these operations. Previews over 40 KB are refused rather than
applying an edit whose complete diff was not displayed.

The implementation is in `modules/pi/files/extensions/lsp-refactor/`.
`modules/pi/lsp-refactor.patch` adds the refactoring interface to the **same**
manager/client pool used by pi-lsp's existing diagnostics and navigation.
`modules/pi/extensions/pi-lsp.nix` fetches and patches immutable pi-lsp 0.1.7 source;
the npm package remains pinned in `modules/pi/default.nix` for its
runtime dependencies, with its original extension disabled to prevent duplicate
servers and tool registrations. Update both pins and revalidate the patch when
upgrading pi-lsp. No installed npm source is modified.

Apply Home Manager and restart Pi to install the patched extension and disable the original together. `/reload` alone does not install this change. On first startup, allow Pi's normal package installation to finish before using LSP.

Run `bash modules/pi/tests/run.sh lsp-workspace-edit.test.ts lsp-refactor.test.ts` for filesystem safety and tool-flow tests. These tests use temporary files and an injected backend to cover preview/apply, action resolution, stale edits, session invalidation, cancellation, and synchronization failures. A live smoke check should rename a symbol used in another file, apply an organize-imports action, inspect the result through `lsp_hover`, and verify that editing a file between preview and apply rejects the stale edit.

## Task, analysis, and artifact extensions

These four extensions have independent entry points and do not replace Pi's built-in tools or its JavaScript `codemode`. Home Manager installs their directories together with Python and ast-grep. Apply Home Manager, then restart Pi; `/reload` alone does not install newly declared extension paths or binaries.

| Extension | Model tools | Interactive commands | Purpose |
| --- | --- | --- | --- |
| `modules/pi/files/extensions/phase-tasks/` | `task_list` | `/tasks` | Phase-aware work tracking with current-branch restoration |
| `modules/pi/files/extensions/python-kernel/` | `python_cell` | `/python status`, `/python reset` | Persistent Python cells, captured output, and images |
| `modules/pi/files/extensions/ast-transform/` | `ast_search`, `ast_preview`, `ast_apply`, `ast_discard` | Model tools | Structural search and previewed, snapshot-checked rewrites |
| `modules/pi/files/extensions/session-artifacts/` | `artifact` | `/artifacts` | Durable text snapshots with listing, paging, and search |

Python execution has the same filesystem and network permissions as Pi; it is not a sandbox. AST previews require an explicit scope and do not modify files until applied. Artifact storage is local and can retain sensitive command output; register only material appropriate to keep with the session.

### Phased tasks

`task_list` accepts `init`, `start`, `done`, `drop`, `append`, `remove`, and `view`. Initialize with `phases: [{ name, tasks: ["task name"] }]`, then explicitly start a pending task. Only one task runs at a time; completing, abandoning, or removing it promotes the next pending task. Targets use stable IDs or unambiguous exact names. Replacing an existing plan requires `replace: true`. The compact widget shows current or next work and disappears when no unfinished tasks remain; `/tasks` opens a read-only, scrollable tree.

Snapshots belong to the current session branch and restore on resume, fork, reload, and tree navigation. Task tracking does not automatically trigger another model turn. Limits are 32 phases, 200 tasks, and a 32 KB snapshot.

### Python cells

`python_cell` accepts `code`, optional integer `timeout` (1–120 seconds, default 30), and `reset`. A lazy `python3` subprocess preserves variables, functions, future imports, and top-level `await` across serialized cells. Final expressions, stdout, stderr, and tracebacks are returned. `display_image` accepts PNG/JPEG bytes, a file path, or a matplotlib figure when matplotlib is already installed; libraries are not installed automatically.

Python exceptions fail the tool while preserving earlier mutations. Timeout, cancellation, process failure, reset, session replacement, and tree navigation discard the namespace; interrupted code is never replayed. Resume and reload start fresh rather than reconstructing historical Python state. Text output and images are bounded; discarded excess text is not an artifact. Python can allocate memory, access files/network, and launch processes with the user's privileges. Its file writes do not participate in Pi's mutation queues.

### Structural transformations

`ast_search` and `ast_preview` require `files`, `language`, and `pattern`; preview also requires `replacement`. Scope is 1–32 explicit existing UTF-8 files inside the working directory, not directories or globs. Search reports zero-based byte ranges and one-based lines/byte columns. Rewrites use ast-grep syntax captures, not LSP symbol identity.

Preview leaves files untouched and returns a complete diff plus a token. `ast_apply` takes `previewId`, locks the scoped files, checks every snapshot, and applies the stored replacement bytes without rerunning the query. Tokens are single-use even on failure, expire after ten minutes, and are discarded on session navigation, reload, or cancellation. `ast_discard` rejects an unused proposal. Symlinks, hardlinks, stale files, overlapping edits, unsupported encoding, and oversized scopes are rejected. Guarded rollback avoids overwriting detected external edits, but external editors do not share Pi's locks: the final check-to-rename race and process crashes prevent cross-file transactional guarantees.

### Retained text artifacts

`artifact` provides `register`, `list`, `read`, and literal `search`. Registration takes an explicit `path` and optional title/line range; later reads use the registered ID. Paging returns `nextOffset` and `nextColumn` so long Unicode lines can be continued. `/artifacts` opens the selector and read-only content viewer; `/artifacts register PATH`, `read ID [OFFSET]`, `search ID TEXT`, and `help` also work. The registration command treats its remaining argument as the path, without shell quoting.

Snapshots are private local copies under Pi's agent directory in `session-artifacts/<origin-session-hash>/`. They survive source deletion and shutdown. Branch-local index entries restore on resume and fork; forks can still reference the originating session's files. No automatic deletion occurs, including when a session is deleted. Close Pi before manually removing unneeded owner directories, and check fork references first. Limits are 2 MiB per source, 20,000 snapshot lines, and 64 files/32 MiB per originating session. Cancelled registration can leave an unindexed retained snapshot that counts toward this quota.

Automatic retention recognizes Pi 1.5.2's native `bash` and `codemode` full-output metadata only after checking builtin provenance and private temporary-file paths. The managed `pi-bg-bash` override is excluded; register its logs explicitly. Free-text paths are never scraped, and other directories are not scanned.

Run the focused feature checks with Python and ast-grep on PATH:

```bash
bash modules/pi/tests/run.sh phase-tasks.test.ts python-kernel.test.ts ast-transform.test.ts session-artifacts.test.ts
```

For an interactive smoke check, use a disposable directory: initialize and advance tasks, inspect `/tasks` at narrow width, evaluate a variable across two Python cells, preview and apply a two-file structural replacement, then register a log and read it after deleting its source. `/python reset` must clear the variable, and an external edit between AST preview and apply must reject the stale proposal before changing another target.

## MCP integration

`files/mcp.json` configures Pi's built-in MCP extension with:

- `nixos`, provided by the installed `mcp-nixos` executable
- `context7`, using its remote MCP endpoint

Pi connects these servers in the background at session startup. Their default `codemode` exposure enables the built-in `codemode` tool, whose scripts discover and call server tools without loading every schema into the prompt. Use `/mcp` for connection status and `pi mcp list` for a standalone connection check. The module does not install `pi-mcp-adapter`, its `mcp`/`mcpScript` tools, or its footer status.

Cloudflare operations use the `cf` CLI installed by the common Home Manager profile. See [Cloudflare CLI setup](../../README.md#cloudflare-cli) for authentication and command discovery. Restart Pi after changing its MCP configuration.

## Theme

Pi selects the generated `one-half-light` theme from `themes/exports/pi/one-half-light.json`. The module also installs `claude-like` as an alternative. The built-in `system` theme remains available in `/settings` to derive colors from the terminal's background and ANSI palette.

The integrated prompt statusline uses the active Pi theme for its background, path, model, and thin rules. Context and task-selection accents use standard ANSI colors from the terminal palette.

## Verification

The test runner requires Nix, Bun, Node.js, jq, ripgrep, and an installed Pi matching the version in `packages/apps/pi/package.nix`. It locates the Nix-installed runtime through `pi` on PATH; for another installation layout, set `PI_TEST_NODE_MODULES` to that runtime's node_modules directory. It evaluates `modules/pi/tests/module-config.nix` with the root flake's pinned Nixpkgs and Home Manager libraries to obtain the generated common settings and extension links for both Herdr conditions, without building packages. It copies the module to a temporary directory, links the installed dependencies there, and uses a temporary Pi agent directory. It does not install dependencies or change the checkout, credentials, or live settings. Pass test filenames to run a subset:

```bash
bash modules/pi/tests/run.sh module-config.test.ts extension-loading.test.ts subagent-profiles.test.ts
```

The suite exercises native Node/jiti discovery for the prompt UI. The patched LSP loading check additionally requires an unmodified, already installed pi-lsp package at the configured version, with its npm dependencies available in the parent node_modules directory. Enable that check explicitly:

```bash
PI_LSP_PACKAGE_DIR="$HOME/.pi/agent/npm/node_modules/pi-lsp" \
  bash modules/pi/tests/run.sh extension-loading.test.ts
```

Without `PI_LSP_PACKAGE_DIR`, only the patched LSP loading check is skipped. It applies `modules/pi/lsp-refactor.patch` to a temporary source copy, adds the local refactoring code, and verifies discovery and single registration of the existing and refactoring tools using Pi's real loader. It does not start a language server or call a model; live server behavior still requires the LSP smoke check above.

After changing this module:

1. Validate edited JSON files with `jq -e . <file>`.
2. Run `bash modules/pi/tests/run.sh` and `bash modules/pi/tests/sync-settings.sh` for extension and device-local settings checks. Run `nixfmt modules/pi/default.nix` when the Nix module changes.
3. Run `nix flake check --no-build` when module wiring changes.
4. Apply the Home Manager configuration manually:

   ```bash
   home-manager switch --flake .#<environment>
   ```

5. Confirm the runtime configuration:

   ```bash
   pi --version
   pi list
   ```

6. In Pi, edit a supported disposable file and confirm that its LSP diagnostics
   are appended to the tool result. Also verify `lsp_hover` or `lsp_symbols` on
   that file.
7. Verify `/fast status`, MCP discovery, structured questions, and the
   web-access tools relevant to the change.
8. In a fresh session, verify `web_enable` and `subagents_enable`, then activate each and confirm that the corresponding tools appear. Verify `/mcp`, `pi mcp list`, and a read-only MCP call through `codemode`, with no extension-loading warnings.

For the subagent rollout, also run `/subagents-doctor` and `/subagents-models`. Inspect `/subagents researcher prompt` and `/subagents reviewer prompt` to confirm the local definitions, then verify one foreground `scout`, one `researcher`, and a read-only `reviewer` before trying `worker` against a disposable fixture. Researcher must activate the web tools, search, fetch an original source, and return citations without creating repository notes. Reviewer must inspect a disposable diff and run a known side-effect-free check, report its actual outcome, and leave files and the index unchanged. These single-child checks should not create an additional Git worktree. To validate the expanded execution policy, use a disposable repository and explicitly authorized parallel workers in separate worktrees, then check a background completion and a review/feedback follow-up. Verify the actual model/thinking, edit isolation, child results, and process termination; do not use live user work as a fixture. A passed settings, scheduler, or frontmatter check alone does not establish child tool loading or model behavior.

Do not commit credentials, OAuth tokens, API keys, `auth.json`, session files,
or MCP credential caches. Pi has no built-in sandbox; packages, extensions,
skills, and tool commands run with the current user's permissions.
