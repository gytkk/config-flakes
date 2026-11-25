# Pi coding agent module

This module manages the global Pi coding-agent setup used by this repository.
It installs Pi and the NixOS MCP server, then exposes tracked configuration
under `~/.pi/agent/` through generated files and Home Manager out-of-store
symlinks.

## Managed resources

| Repository path | Runtime path | Purpose |
| --- | --- | --- |
| `modules/pi/files/settings.json`, `modules/pi/files/settings-devsisters.json`, and `modules/pi/default.nix` | `~/.pi/agent/settings.json` | General settings and package selection merged into a device-local writable file |
| `files/web-search.json` | `~/.pi/web-search.json` | Web Access defaults |
| `modules/pi/files/interactive-shell.json` | `~/.pi/agent/interactive-shell.json` | Lean, on-demand shell tools; semantic transmission disabled |
| `modules/pi/files/lens.json` | `~/.pi-lens/config.json` | Lens policy and LSP launcher overrides |
| `modules/pi/files/seed-mcp.sh` | `~/.pi/agent/mcp.json` | Writable per-device server configuration; activation adds missing shared servers |
| `modules/pi/files/models.json` | `~/.pi/agent/models.json` | Context-window overrides for the configured OpenAI chat models |
| `agent-core/rules/` and `agent-core/adapters/pi.md` | `~/.pi/agent/AGENTS.md` | Generated shared and Pi-specific instructions |
| `agent-core/rules/OPERATING.md` | `~/.pi/agent/APPEND_SYSTEM.md` | Operating invariants added to Pi's system prompt |
| `files/extensions/` | `~/.pi/agent/extensions/` | Local Pi extensions |
| `modules/pi/agents/researcher.md`, `modules/pi/agents/reviewer.md`, and `modules/pi/agents/worker.md` | `~/.pi/agent/agents/` | Specialist prompts, tool contracts, and worker completion schema |
| `themes/exports/pi/one-half-light.json` | `~/.pi/agent/themes/one-half-light.json` | Generated One Half Light theme |
| Agent core skill sources | `~/.agents/skills/` | Codex/Pi shared pool, installed or updated by shared activation |

The module also installs:

- `pkgs.pi`, unless `modules.pi.installPackage` is off ([jari workers](#jari-workers))
- `pkgs.mcp-nixos`

When session recording and its Pi provider are enabled, Home Manager installs a shutdown extension without changing Pi settings. It passes only the persisted session's ID and path to the [session recorder](../agent-session-record/README.md). `--no-session` and session files outside the default store are skipped.

`settings.json` is a private, writable regular file on each machine, not a checkout symlink. The Devsisters MacBook, Mac Studio, and Jari worker select `modules/pi/files/settings-devsisters.json`. The parent and native subagents default to `claude-sdk/claude-opus-5-5`, subagents reject models outside the Claude SDK provider, and bundled external CLI runners are disabled. Sign in with Claude Code on each Devsisters device. Trusted project settings can override these device defaults. Jari keeps its worker-pinned Pi binary. Other devices use `modules/pi/files/settings.json`.

Nix combines the selected settings with the `extensionPackages` selection in `modules/pi/default.nix` to generate the common settings. Home Manager merges these into the local file before linking the new generation; common top-level keys take precedence, while other local keys are preserved except the retired managed `powerline`, `bashMode`, and `workingVibe` keys. Pi creates `deviceId` on first ChatGPT login and keeps it, `trackingId`, and `lastChangelogVersion` local. The sync rejects these fields in common settings. Existing symlink installations are migrated without writing their local state back to the checkout.

`/settings` and package commands modify only the local settings file. Changes to managed keys last until the next Home Manager application; edit the selected settings file for general defaults or `extensionPackages` in `modules/pi/default.nix` for package selection. Close Pi before applying Home Manager to avoid concurrent settings writes. Other mutable configuration still uses out-of-store symlinks. Generated instructions and common settings change only after applying Home Manager.

This repository's `.pi/settings.json` is a relative symlink to `modules/pi/files/settings.json`, so trusted Pi sessions in this checkout use the shared settings as project overrides, including on Devsisters devices. Changes to the shared file are reflected through the link without copying; editing the project file also edits the shared source.

## Extension selection

`modules/pi/default.nix` declares the managed global extension selection in two places:

- `extensionPackages` lists Pi package sources, version pins, and resource filters in Pi's settings format. It supplies the generated `packages` setting; do not duplicate that key in `modules/pi/files/settings.json`.
- `localExtensions` maps installed extension names to checkout-backed or patched sources. Home Manager creates their links under `~/.pi/agent/extensions/`. The conditional Herdr state integration is declared beside this list.

Supporting files such as `modules/pi/files/extensions/subagent/config.json` remain separate from the extension entry points. Lens loads only from its pinned npm package; there is no local LSP or AST implementation or patched-source loader. These lists do not enumerate Pi's built-in extensions or resources added by project settings or command-line arguments.

## Hindsight memory

The Pi package owns the companion skill. Older enabled sessions also copied it into `~/.pi/agent/skills/hindsight-coding-agent`. If Pi reports a collision, close those sessions, move the legacy copy outside Pi's discovery paths, and leave the package copy in place. The shared installer accepts an empty `~/.pi/agent/skills` directory and leaves it untouched. A symlink or a directory with any remaining entries still requires an ownership handoff before switching configurations.

On the five managed Hindsight clients, Home Manager installs `~/.hindsight/coding-agent.json` as an out-of-store symlink into the configuration checkout. All five link to the [same Pi memory settings](files/hindsight-client.json). Jari installs the same settings as an opt-in preset with its worker checkout mapped to `config-flakes`; see [jari workers](#jari-workers). Pi enables memory for all repositories by default, including work and client repositories. `~/development/config-flakes` keeps its explicit `config-flakes` bank; unmapped repositories use the Git project name without the `coding-agent::` prefix. To keep using an existing prefixed bank, add the unprefixed ID as an alias before restarting Pi; changing the template does not rename existing banks. Pi recalls relevant memories on the opening prompt (all memory types, low budget, up to 1,000 tokens) and retains the active branch after completed responses. For later questions about past decisions or attempts, the tool guide asks Pi to search memory and verify the result against current code. Untouched sessions are not imported and Git ingestion remains disabled.

Before the first switch, move any existing unmanaged config or symlink to a private backup outside the checkout. It may contain a bearer token or settings for another harness. Do not copy it into Git or force the managed link over it. Review any settings you want to keep separately. Once linked, edits through the home path modify the checkout; never put a token there. Agenix supplies the credentials at runtime. To keep private overrides, point `HINDSIGHT_CONFIG` at a separate private file instead.

The managed hosts run both Pi and the stock `hindsight` CLI over private routes. The Macs, onyx, and provisioned jari workers use sepia's [Tailscale Serve endpoint](https://pylv-sepia.tailbbb9bf.ts.net). On denim both clients connect to `192.168.0.2:443` and verify TLS for `hindsight.pylv.dev`. Pi pins the connection address; the CLI uses bubblewrap to mount a process-local hosts file, without changing Windows or WSL DNS. Sepia uses its own loopback API. The Pi extension maps the managed origin in the shared JSON to the host's route before attaching credentials. No client needs an SSH forward or a Cloudflare Access service token.

Apply [the sepia service configuration](../../systems/hosts/pylv-sepia/README.md) before switching clients. After applying Home Manager, run `hindsight bank list`, then `hindsight document list config-flakes` to browse saved documents. The CLI wrapper reads the runtime API key for each invocation; it is not exported to the login shell. Restart Pi and run `hindsight_diagnose` to check its effective endpoint and credentials.

## Korean conversation translation

`modules/pi/files/extensions/korean-translation/` enables Korean/English translation in parent TUI sessions. It uses `openai/gpt-6-luna` through Pi's model registry and existing authentication, independently of the selected working model. `/translate off`, `/translate on`, and `/translate status` control the current branch; the setting survives reload, resume, and fork. New sessions start enabled. RPC, JSON, print mode, and subagent processes are unchanged.

For interactive Korean input, the extension displays and stores the original in a custom entry, then submits the English translation as the native user message. Both are visible in the transcript, but only the English message enters model context. English-only input passes through. Slash-command names stay unchanged while Korean arguments can be translated before Pi expands skills and templates; commands handled before the input hook and shell commands are not translated. Images remain attached and are not sent to the translator. Extension-injected prompts, system instructions, file contents, tool calls/results, and thinking blocks are not translated.

The working model is asked to write conversational prose in English without changing the required language of generated artifacts. Its response streams normally. After each completed turn, including any tool execution, the extension appends a Korean translation as a TUI-only custom entry, with a divider immediately followed by the Korean text and no translation heading. It preserves the English assistant message and excludes the Korean display entry from future model context and compaction input. Saved translation entries render again without model calls; the extension does not backfill older conversations.

Fenced code, inline code, recognized URL/path tokens, and slash-command prefixes are replaced with placeholders before translation and restored byte-for-byte afterward. Placeholders may move to accommodate natural grammar, but each original placeholder must appear unchanged exactly once. A leading slash command must remain at the start with its argument boundary intact. Missing, duplicated, altered, or unknown placeholders reject the translation. Use code formatting for commands, exact literals, and paths with spaces or non-ASCII characters. These checks preserve literal content, not its relationship to surrounding prose; translation is model-generated and may still change meaning. The originals remain available for comparison.

Input translation has a 60-second deadline. Response translation allows up to three minutes. Both accept at most 64 KiB of source text. A placeholder validation failure triggers one retry from the original protected text with a preservation reminder, within the same deadline and cancellation signal. Other failures are not retried. Escape cancels the input loader or an active output translation. A failed input translation blocks submission and restores the text when the editor is empty; it never overwrites a newer draft. A failed output translation keeps the English response and reports a warning, including when the retry also fails. Missing Luna authentication does not fall back to another model. Translation adds latency and provider usage; successful translations record usage from both attempts when retried in custom-entry metadata, but this usage is not included in Pi's native session cost totals.

Apply Home Manager manually to install the extension, then restart Pi or run `/reload`. Run `bash modules/pi/tests/run.sh korean-translation.test.ts extension-loading.test.ts module-config.test.ts` for isolated translation, cancellation, branch persistence, native rendering, and Node/jiti discovery checks. For a live check, submit a short Korean request containing inline code, compare the preserved Korean input with the English user message, and verify that an English response is followed by a separate Korean translation. Check `/translate off` before returning to untranslated interaction.

## Main settings

`files/settings.json` currently selects:

- `openai/gpt-6.2-astra` with `medium` thinking
- the generated `one-half-light` theme
- a startup header without the loaded-resource listing
- a local status border directly above the open `╰─` prompt; the editor reserves three columns for the prompt while output padding stays at one column
- the hardware terminal cursor for IME positioning
- prompt-cache notices, with cache warming left at Pi's default `"streaming"` mode

The module also exports `PI_CACHE_RETENTION=long`, because Pi has no settings key for cache retention. Pi then asks Anthropic for a 1-hour prompt cache instead of the 5-minute default, and OpenAI Responses models for their extended retention. A 1-hour Anthropic cache write costs more than a 5-minute one, but a session that pauses longer than five minutes rereads its prefix from the cache instead of writing it again. Restart Pi after applying Home Manager; subagents inherit the variable.

Pi uses the default fullscreen mode and built-in keybindings: `Home` and `Ctrl+A` move to the input line's start, while `Ctrl+Home` moves to the top of the fullscreen conversation. The module does not install a keybindings file. The custom editor inherits Pi's input shortcuts; use `/attach` for shell sessions rather than the removed Down/Enter task item.

### File links in Herdr

In fullscreen Pi running inside Herdr, Ctrl-clicking a local `file://` link, including the paths in `read` and `edit` tool headings, splits the Pi pane to the right and opens the file in Neovim. Keep Ctrl held until releasing the mouse button. The new pane takes focus, retains Pi's working directory, and closes when Neovim exits. Each Ctrl-click creates a new pane. Plain file-link clicks are consumed without launching Neovim or the platform's default editor, such as Zed. In Herdr App and Herdr TUI, dragging selects text in white on teal, including syntax-highlighted output. In other terminals, Pi keeps its default reverse-video selection. Web links retain their normal click behavior; outside Herdr, Ctrl-clicked file links use the platform's default handler.

This behavior comes from the [local Pi package patch](../../packages/apps/pi/herdr-file-links.patch), not an extension or a Herdr Ctrl-click handler. It requires `nvim` on `PATH` and a working Herdr CLI. Invalid or remote file URLs are rejected. Launch failures are reported on Pi's stderr without falling back to another editor; if command submission fails after splitting, the new pane remains open for inspection.

Apply Home Manager and restart Pi to load the patched package; `/reload` alone does not replace Pi's code. Run `bun test packages/tests/pi-file-links.test.ts` from the repository root for routing, path quoting, and failure tests. To test mouse routing against a patched upstream source tree with its runtime dependencies available, run `PI_TUI_SOURCE=/path/to/patched/packages/tui/src bun test packages/tests/pi-file-clicks.test.ts`; this suite is skipped when `PI_TUI_SOURCE` is unset. After applying, confirm that a plain click on a file path in either a `read` or `edit` heading launches no editor, Ctrl-click opens the right-hand Neovim pane, and `:q` closes it. Regular mode delegates mouse input to the terminal and is not covered by this fullscreen click policy.

### Context windows and automatic compaction

`modules/pi/files/models.json` sets the configured OpenAI chat models, `gpt-6.2-astra`, `gpt-6.2-sol`, and `gpt-6-luna`, to a 1,000,000-token (1M) context window through `modelOverrides`. The overrides apply to parent and subagent sessions without changing authentication, endpoints, output limits, or other catalog metadata. They do not change the separate Luna classifier or the unused legacy `openai-codex` provider. This is Pi-side metadata for usage and compaction calculations, not a server capability change; the endpoint must accept the larger context.

Apply Home Manager to install the model configuration, then restart Pi. The local statusline shows the effective limit beside context usage, for example `11% / 1M`, and updates when the selected model changes. It uses the current usage metadata or the selected model before usage is known, rather than a hard-coded 1M label.

`compaction.modelOverrides` configures `openai/gpt-6.2-astra` to reserve 54,400 tokens and retain roughly 40,000 recent tokens without summarization. With the 1,000,000-token context window, automatic compaction starts above 945,600 tokens. Other models use Pi's default compaction budgets. The reserve remains an absolute token count.

### ChatGPT authentication

Run `/login openai` in Pi and choose `Sign in with ChatGPT`, then approve the browser login. Pi stores the OAuth credential under `openai` in `~/.pi/agent/auth.json`. Verify without printing credentials:

```bash
pi auth check --provider openai --json --no-refresh
```

The parent and delegated chat models use `openai`. The legacy `openai-codex` provider is not used. On existing installations, remove its saved credential through `/logout` and select `openai-codex`; its models then disappear from the authenticated model picker without affecting the `openai` login or historical sessions. In Pi 1.1, its GPT-6 Luna classifier is a separate model type and requires an API key; ChatGPT OAuth supports the chat model but not the Decisions API classifier. Use the existing `with-openai` wrapper in a separate process for API-key clients rather than replacing the subscription credential. See the [OpenAI API module](../openai-api/README.md).

### Claude subscription

Pi's built-in `/login anthropic` subscription login is billed as per-token extra usage, not Claude plan limits. `pi-claude-subscription` instead sends Claude requests through the Claude Agent SDK, which runs Claude Code with its own login, so sign in with `claude` and `/login` first; the extension does not read Pi's credentials. Select a model such as `/model claude-sdk/claude-sonnet-5-5`. The extension uses Claude Code's system prompt by default because Anthropic bills requests with Pi's prompt as extra usage, disables Claude Code's built-in tools so only Pi's tools run, and writes session transcripts under `~/.claude/projects`. Anthropic can change this routing at any time, and subscription use through the Agent SDK remains subject to Anthropic's terms for the plan.

`pkgs.pi` calls the official upstream Nix package definition from a pinned source release. [App package maintenance](../../packages/README.md#updates-and-ci) owns that source pin. Apply Home Manager to install the pinned Pi version and synchronize common settings.

`files/web-search.json` sets the default Web Access workflow to `none`, so
ordinary searches return directly without opening the curator. Requests that
need human source selection can still set `workflow: "summary-review"` or use
`/curator` explicitly.

The `extensionPackages` list in `modules/pi/default.nix` is intentionally version-pinned:

| Package | Version | Purpose |
| --- | --- | --- |
| [`@juicesharp/rpiv-ask-user-question`](https://github.com/juicesharp/rpiv-mono/tree/main/packages/rpiv-ask-user-question) | `2.15.0` | Structured user questions from rpiv-mono |
| [`@juicesharp/rpiv-todo`](https://github.com/juicesharp/rpiv-mono/tree/main/packages/rpiv-todo) | `2.15.0` | Task list, dependencies and live widget |
| `pi-interactive-shell` | `0.18.0` | Interactive PTY overlay, background dispatch, and completion notifications |
| `pi-lens` | `4.4.12` | LSP diagnostics/navigation, semantic rename, AST search and replacement preview |
| `pi-web-access` | `0.46.0` | Web search, source checks, and content fetching |
| `pi-subagents` | `0.84.0` | Foreground-first delegated Pi sessions |
| [`pi-claude-subscription`](https://github.com/jkeskikangas/pi-claude-subscription) | `0.2.4` | Claude models through the Claude Agent SDK and the Claude Code login |

Versioned npm packages are skipped by `pi update --extensions`. Update pins
through a reviewed repository change, and review package source and release
notes because Pi packages execute with the permissions of the Pi process.

`pi-subagents` loads only its extension; shared skills and the Pi adapter own delegation guidance instead of bundled skills and prompt templates. It uses packaged agents plus local `researcher`, `reviewer`, and `worker` definitions with profile-specific model routing. Delegation defaults to foreground and stores artifacts under the parent Pi session. Foreground runs and single-agent background runs have a default runtime limit of 120 minutes. An explicit run timeout or single-agent timeout overrides this default. Authorized tasks may use parallel or background children, isolated worktrees, and review/feedback follow-ups through explicit native resume. Nested delegation, workflow scripts, missions, schedules, automatic resume on reload, and the generic `delegate` agent remain disabled. Tool activation stays dynamic through `subagents_enable`.

In the default profile, delegation defaults to `openai/gpt-6.2-sol`; `worker` uses this model with `high` thinking, while `reviewer` uses `openai/gpt-6.2-astra` with `xhigh` thinking. `scout` and `researcher` use `openai/gpt-6-luna`, `planner` uses Sol, and `oracle` uses Astra. All three models are in the allowlist.

The packaged `planner` and `oracle` defaults are overridden to fresh context. The local researcher, reviewer, and worker also use fresh context. Use an explicit `context: "fork"` only when the child genuinely needs the parent transcript; otherwise pass a compact task contract. The researcher skips inherited project instructions because its role prompt and research task provide its operating context. Other agents retain repository instruction inheritance, but fresh children exclude the operator's global context and skills catalog unless explicitly configured otherwise. The child runtime also excludes the operator's `APPEND_SYSTEM.md`; a parent must pass relevant constraints in the task contract rather than assume its whole prompt is inherited. When a child persists a substantial artifact, prefer `outputMode: "file-only"` so the full result is not injected back into the parent context.

Only `worker` retains normal implementation tools. `scout`, `researcher`,
`context-builder`, and `reviewer` have no `edit` or `write` tool. Their `bash`
access is for inspection and verification and is not a security boundary; Pi
packages and child processes still run with the current user's permissions.
The Pi adapter permits execution choices within an already authorized task, not unsolicited delegation or broader authority. Concurrent implementation workers use separate worktrees with exclusive edit ownership; each checkout has one writer, and shared-file changes are integrated sequentially. Launch and runtime failures must be reported and diagnosed before a bounded retry is proposed under the tool's recovery rules. Model, provider, or execution-mode changes are not silent fallback paths. Background completion uses native notifications; the parent reconciles every child and verifies its evidence before integration.

`modules/pi/files/extensions/subagent/config.json` sets a run-wide concurrency limit of 8, with up to 16 tasks per parallel group and default parallel concurrency of 8. The session-wide cumulative spawn cap is disabled (`maxSubagentSpawnsPerSession: 0`); the package's separate per-run budget remains in effect. These are package execution limits, not a machine-wide process quota. `asyncByDefault: false` and `forceTopLevelAsync: false` preserve foreground defaults without prohibiting explicit background calls. `autoResume: false` prevents automatic revival on reload; explicit native resume remains available for authorized follow-ups. The managed `disabledFeatures: ["workflow-scripts"]` setting keeps native `tasks` and `chain` orchestration available and routes `/run` directly to a child compatible with Hub.

Apply Home Manager to refresh generated instructions, then restart Pi in a new session rather than resuming a session with the old policy. Model defaults and the allowlist are unchanged; pass an exact provider/model and thinking suffix when a task requires one. Agent Hub and the Herdr sidebar still have their documented foreground-oriented coverage; use package Fleet/status for other background runs. Enabling background execution does not extend those local UIs.

### Execution profiles

The local definitions are thin execution profiles: they specify tools, context inheritance, child extension loading, role restrictions, and completion output. Each delegation request supplies the objective, allowed scope, starting evidence, completion conditions, and relevant operating constraints. The prompts retain a short baseline because fresh children do not inherit the parent's global instructions or skills. The worker's structured completion contract remains available to Agent Hub.

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

The [Pi adapter](../../agent-core/adapters/pi.md#progress-and-final-replies) directs routine progress to the task widget. Routine replies lead with the result and usually take one or two sentences. Pi adds detail when requested or needed to explain evidence, risks, or next steps.

`agent-core/rules/OPERATING.md` is rendered as `APPEND_SYSTEM.md`, so it augments Pi's maintained system prompt instead of replacing it. The generated `AGENTS.md` combines shared methodology, prose guidance, and `agent-core/adapters/pi.md` without duplicating the operating invariants.

The shared [agent skills module](../agent-skills/README.md) links selected repository-owned skills individually into `~/.agents/skills/` and uses the Skills CLI for external skills, including when only Pi is enabled. Use the installed Pi documentation for skill development and runtime APIs. The `pi-subagents` extension provides delegation independently of any reference skill.

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

The progress payload and slash events follow `pi-subagents` 0.84.0. Review this integration when updating that package. It does not import the package's internal modules or modify its execution behavior. If a metadata update times out without an error message, Pi waits for the next update before warning. Consecutive timeouts, a timeout during shutdown, and other command errors produce a warning without interrupting Pi.

Home Manager installs the extension. After applying the configuration, run `/reload` in Pi and `herdr server reload-config` for the [Herdr sidebar layout](../herdr/README.md#subagents). For a foreground smoke test, run `/run scout List the top-level files without changing anything` inside a Herdr Pi pane and verify that the child appears, updates, and disappears when finished. Run `bash modules/pi/tests/run.sh herdr-subagents.test.ts` for the isolated event and metadata tests.

### Herdr Desktop

The same extension publishes foreground `subagent` and `/run` child metadata using the parent core terminal's `pi.subagents` label. This repository does not install a Desktop mod to display these labels. `HC_TERMINAL_ID` identifies the terminal; `HC_TERMINAL_SOCKET` takes precedence over `HC_SOCKET`. A valid Herdr TUI pane uses only pane metadata, never the enclosing terminal label. Headless sessions and `PI_SUBAGENT_CHILD=1` processes do not report.

Snapshots refresh every five seconds. Session replacement gets a fresh identity; shutdown removes the label after queued writes finish. Core labels are persistent, so activity excerpts can remain on disk after a crash until replaced or cleared. Run `bash modules/pi/tests/run.sh herdr-subagents.test.ts` for the reporter tests.

### OpenAI fast mode

`modules/pi/files/extensions/codex-fast-mode.ts` provides:

- `/fast [on|off|status]`
- `service_tier: "priority"` for `openai` requests while enabled
- fast-mode status for the local prompt statusline

Fast mode starts disabled in each new session; use `/fast on` to enable it. Toggle changes are stored in the session so resumed branches recover their previous state. The extension owns the priority-mode request field and publishes its display state through `codex-fast-mode`. The local prompt UI consumes that status through Pi's footer data provider, without importing the fast-mode implementation. The display value is `fast` only while enabled for an OpenAI parent session; otherwise the status is cleared. Model changes refresh it without changing the saved toggle or request behavior.

Subagent child processes are detected through `PI_SUBAGENT_CHILD=1` and do
not receive the priority service tier. `PI_SUBAGENT_PARENT_SESSION` is also set
in the parent UI session for permission forwarding, so it must not be used as
the child-process signal. Parent Pi sessions continue to use fast mode normally.

### Subscription usage

`modules/pi/files/extensions/subscription-usage.ts` adds `/usage` and publishes the weekly subscription quota remaining and time until reset through the `subscription-usage` status key. The quota follows the selected model's provider: `openai` and `openai-codex` models show the Codex subscription, and `claude-sdk` and `anthropic` models show the Claude subscription. Other providers show no quota. Switching models shows the cached reading for the new provider immediately and fetches one when none is due. The compact format is `#######--- 79% (⏳ 3d 14h)`: a ten-cell remaining-quota bar, the remaining percentage, and the reset countdown in days and hours. The countdown uses 24-hour days and drops partial hours; less than one hour remaining appears as `0d 0h`. Each `#` represents a full ten percentage points remaining. The local statusline colors the filled cells green and places the quota immediately before the model. An unknown reset time appears as `(⏳ unknown)`. The statusline shortens or omits the quota at narrow widths to preserve context and model information. This quota is separate from session tokens and API cost. Interactive parent sessions fetch usage at startup without blocking it, then refresh Codex every minute and Claude every five minutes while that provider is selected. `/usage` refreshes the selected provider immediately and shows the local reset date and time without a time-zone suffix, along with the source timestamp. Concurrent refreshes for a provider share one request. Codex requests time out after 30 seconds and Claude requests after 60 seconds. Shutdown or reload cancels pending work. Non-interactive sessions and subagent processes do not poll.

The extension runs `codexbar usage --provider codex --source oauth --json --no-credits` or `codexbar usage --provider claude --source cli --json --no-credits` using the CodexBar CLI already installed by the common profile. It reads the weekly `usage.secondary` window, not session token totals or API credit balances. Codex OAuth uses the Codex CLI account selected by CodexBar and `CODEX_HOME`, which can differ from Pi's ChatGPT login. Use the same account in both clients to track the subscription used by Pi. The Claude source starts a Claude Code session through CodexBar, which takes about 30 seconds, and reports the Claude Code login that `pi-claude-subscription` also uses. CodexBar's Claude OAuth source is not used because it cannot read the Claude Code login that macOS keeps in the Keychain. The extension does not read, copy, or manage authentication files; CodexBar owns usage retrieval and its authentication behavior. It does not fall back to browser cookie extraction.

Failed queries replace the displayed quota with an unavailable status; `/usage` reports a bounded error without raw CLI output or account identifiers. Data older than ten minutes is marked stale. Missing reset times remain unknown, and an elapsed reset is marked as awaiting fresh usage rather than assuming a replenished quota. Usage stays in memory and is not added to the model context or session history.

Apply Home Manager and restart Pi to install the extension. To try it before applying Home Manager, start `pi --extension ./modules/pi/files/extensions/subscription-usage.ts` from the repository root. If a fetch fails, check that `codexbar` is on `PATH` and that the Codex CLI or Claude Code account is authenticated; Pi's `/login openai` does not authenticate CodexBar.

### Image paste labels

Pasting a standalone image path into the editor shows a numbered label such as `[Image #1]`. This covers Herdr's quoted screenshot paths, Pi's native clipboard image insertion, and newline-separated image paths. Labels use Pi's native paste-marker editing, including cursor movement, deletion, and undo. Submission and external editing expand them back to the original paths, including quotes and spaces, so Pi can read the same files. This changes the editor display, not the saved message text or image delivery. Prose, non-image paths, and shell commands remain literal.

The behavior comes from `packages/apps/pi/image-paste-labels.patch`. Close Pi, apply Home Manager, and restart Pi to load the patched package; `/reload` cannot replace the running editor implementation. With an unpatched Pi source checkout matching the installed version, run `PI_SOURCE=/path/to/pi-source bun packages/tests/pi-image-paste.ts` to test the patch and local prompt UI without replacing the live runtime.

### Prompt and cursor rendering

`modules/pi/files/extensions/prompt-ui/` is the sole managed editor/footer owner. It places the working-directory path and Git branch/counts on the left, a context meter spanning the remaining space, and extension statuses, weekly quota, model, thinking, and fast mode on the right. The statusline replaces the editor's top border; the `╰─` input follows directly without an extra horizontal separator or idle bottom border. Narrow widths shorten or omit less important statuses while preserving context and model information.

The directory badge blends a pale mint tint into the theme background; its text and Git colors retain their theme-derived contrast. The model badge keeps its separate theme-derived tint.

Git counts use unstaged (`*`), staged (`+`), and untracked (`?`) files. The watcher waits five seconds after each query finishes before starting the next one, counts individual untracked files, and treats merge conflicts as unresolved work. If a query exceeds its time or output limit, the status line shows `git!` without a notification and retries on the next poll. Other Git errors still produce a warning. Shutdown and reload cancel pending work.

Fast mode and CodexBar retrieval remain independent status providers. The renderer consumes `codex-fast-mode` and `subscription-usage` through Pi's footer data provider and excludes them from aggregate extension statuses. It also works without either provider. The prompt border omits Lens's `LSP Inactive` status while retaining active and failed LSP statuses. Authentication, quota refresh/failure/stale handling, request payloads, child exclusion, and saved fast-mode toggles remain provider-owned.

The custom editor extends Pi's `CustomEditor` and retains native input, history, wrapping, autocomplete, mouse coordinates, and `/compact <text>` semantics. It reserves three padding columns for the prompt instead of adding a prefix outside Pi's coordinate system. It preserves hardware-cursor markers, removes software-cursor styling, and retains scroll hints and autocomplete row offsets. The ordinary footer renders no duplicate status. The UI has no legacy `/bg (N)` selector; interactive-shell and Lens remain installed independently.

Apply Home Manager with Pi closed to remove Powerline from the managed package list and install the local prompt extension together, then restart Pi. Activation removes the retired managed Powerline settings but does not delete npm caches or Powerline's saved queue/stash data. Do not leave an unmanaged Powerline extension enabled alongside the local editor. `/reload` alone cannot change installed extension ownership.

Headless checks exercise the real Pi editor, status rendering across terminal widths, effective model/context values, provider independence, Git polling and disposal, paste/history/autocomplete, native compaction submission, and mouse cursor placement. Node/jiti checks cover discovery and coexistence with installed shell, Lens, and questionnaire packages. Actual Korean IME candidate placement and live overlay focus restoration still require a terminal check.

### Automatic session titles

In a parent Pi TUI, `modules/pi/files/extensions/auto-title.ts` asks `openai/gpt-6-luna` to name an unnamed session when you submit a text request. Naming runs alongside the main task. The extension sends up to 2,000 characters of request text using Pi's configured OpenAI authentication, even when the working model is from another provider. It does not include attached images, tool results, or conversation history.

Pi saves the name and updates its terminal title, which lets Herdr Desktop distinguish sessions in the same repository. The extension makes one naming attempt per loaded session and never replaces an existing name; use `/name` if the task changes. To check it after applying Home Manager, start a new Pi session in Herdr and submit a task. The row should change from `π - <directory>` to `π - <task> - <directory>` without waiting for the task to finish.

### Terminal title on exit

Pi sets the terminal title to `π - <directory>` and does not clear it on exit, so terminal applications can keep showing it after returning to the shell. `files/extensions/terminal-title-reset.ts` sends an empty title when the TUI process exits after a quit. It writes from a process `exit` listener because Pi 1.5 rewrites the title after `session_shutdown`. Session reload, new, resume, and fork leave the title to Pi.

## On-demand tools

Fresh sessions use the packages' own `enable_interactive_shell`, `web_enable`, and `subagents_enable` tools to activate shell, web, and delegation tools when needed. Package commands such as `/run` remain available. Native MCP uses Pi's `codemode` tool without declaring every server tool to the model. The module does not install a custom `enable_tools` tool or `/tool-profile` command.

When migrating an existing installation, apply Home Manager to remove its managed `~/.pi/agent/extensions/tool-profiles` symlink, then restart Pi. New sessions use the default activation state; resumed web/delegation tools may retain transcript activation, while interactive-shell resets its deferred tool on every session start.

## Structured questions

The managed `@juicesharp/rpiv-ask-user-question@2.15.0` is already published from [rpiv-mono](https://github.com/juicesharp/rpiv-mono/tree/main/packages/rpiv-ask-user-question). The repository move did not change its npm identity; keep the exact package pin in `modules/pi/default.nix`. Do not install the monorepo root or `@juicesharp/rpiv-pi` to obtain this one extension: the umbrella package introduces a separate workflow and sibling setup.

Use `ask_user_question` with the package's existing `questions`, `header`, `multiSelect`, and `options[].label`, `description`, and optional `preview` fields. Terminal questions retain the custom-answer row, multi-question review, notes, and Escape cancellation. RPC uses native select/input dialogs with reduced presentation; headless runs remove the tool from the active set. The extension makes no model calls of its own. The module does not select the optional i18n extension. Without that SDK, UI chrome uses its English fallback; user-authored Korean questions and answers are separate from localization.

`rpiv-todo` is selected separately as the task tracker. `rpiv-advisor` and other siblings are not installed implicitly, and the pi-subagents contract remains unchanged. The earlier pi-ask-user replacement is not selected.

## Tool and extension-message cards

`modules/pi/files/extensions/rounded-tools.ts` wraps each tool's call and text result in a thin, rounded terminal outline. It uses Pi's renderer resolver without replacing tool definitions or execution. Existing renderers retain syntax highlighting, edit previews, codemode nested-call status, streaming updates, and expansion controls. The tool name sits in the top border. The outline and title use the theme's warning color while running, success color on completion, and error color on failure. The card does not paint a background. Todo status markers and read error markers carry color without tinting their labels. Custom renderers retain their own highlighting. Panes narrower than five columns omit the border.

Pi renders native image attachments below the text frame. Custom renderers that emit inline Kitty or iTerm2 graphics bypass the frame to preserve terminal image placement. Empty renderers stay hidden, and a failing renderer shows a warning with plain call or result text. Generic tools without renderers show their arguments and a collapsible text result.

The extension also frames every displayed custom message, including background Bash output/completion, subagent results and notices, web-search messages, and future custom message types. The top border displays the message type in the theme's custom message label color. The frame wraps the existing renderer (or Pi's default Markdown renderer) rather than replacing the message body, so custom controls, links, expansion, and explicit inner colors remain intact. Assistant/user messages, user-entered shell commands, dialogs, and custom entries registered through `registerEntryRenderer` are separate surfaces and are not changed.

Lens owns its tool and diagnostic rendering. The generic frame delegates to Lens's compact renderer and preserves expanded results; no legacy diagnostics payload parser or dedicated notice renderer remains. Missing servers and unconfirmed diagnostics retain their upstream explanation, not a fabricated clean summary.

The local `packages/apps/pi/message-renderer-wrappers.patch` adds `pi.registerMessageRendererWrapper(wrapper)`. Its TUI-only callback receives `(component, message, options, theme)` and returns a component. Wrappers run after the original renderer or its default fallback, in extension load order and then registration order. Expansion, theme changes, and history restoration rebuild the wrapped component. A throwing wrapper shows a warning and preserves the preceding component; the hook does not alter session data, model context, or non-TUI output.

Apply Home Manager to install the patched Pi package, then restart Pi. `/reload` cannot load a package-level API change. On older Pi, tool frames remain available and an upgrade warning explains why custom messages are not framed. Run `bash modules/pi/tests/run.sh rounded-tools.test.ts custom-message-frames.test.ts` for generic rendering, expansion, links, mouse coordinates, narrow panes, theme changes, and image placement. The Lens integration test also renders a real package replacement preview inside the frame.

## Background shell tasks

Pi's built-in `bash` handles short non-interactive commands, including its native exit and timeout results. A collapsed Bash tool block shows the first line of a multiline command followed by `…`. Press `Ctrl+O` to toggle tool expansion, or click a block with a result to toggle just that block. Command output keeps its usual preview. `pi-interactive-shell` loads only its extension (bundled skills and prompts are excluded), without overriding `bash`. The managed `modules/pi/files/interactive-shell.json` selects `defer: true`, `toolSchema: "lean"`, and `jev.enabled: false`. Call `enable_interactive_shell` when needed; `interactive_shell` becomes available on the next turn. Reload, new, resumed, and forked sessions reset activation. A CLI tool allowlist must include both names.

For long builds and tests, use a raw command in background dispatch:

```javascript
interactive_shell({
  command: "npm test",
  mode: "dispatch",
  background: true,
  handsFree: { autoExitOnQuiet: false }
});
```

The explicit quiet opt-out is required: upstream dispatch otherwise stops supervision after a 15-second startup grace and 8-second quiet threshold. Silence is not process completion. Wait for the package's completion notification and inspect its exit code and `completionReason`; do not rerun the command or poll to recover results. Monitor mode is only for a concrete output/state condition. Semantic supervision and selected-output transmission remain disabled; do not opt into them in tool calls.

For input-driven CLIs, use the interactive overlay and send `input` with `submit: true` (or named `inputKeys`) against the returned session ID. The user can type to take over a monitored overlay, use Ctrl+G to return monitoring control, Ctrl+B to background, Ctrl+T to transfer output, or Ctrl+Q for the session menu. `/attach` lists retained sessions; `/attach <id>` opens one, and `/dismiss <id>` stops/removes it. The package owns the background widget and overlay; there is no `/bg` compatibility UI or Down/Enter task shortcut.

Timeout, local supervision cancellation, actual process exit, and process-tree termination are different outcomes. `kill: true` reports cancellation and attempted termination, not confirmed exit. Running background tasks survive `/reload`; quitting Pi or replacing the session attempts cleanup. Do not promise survival after Pi exits. Headless completion saves terminal output under Pi's agent cache (`cache/interactive-shell`); the notification names the file, and a later session-ID query can point to it even when the active session is gone. Upstream prunes saved logs older than 24 hours on later writes. Use `artifact` with an explicit log path for durable retention. Native bash automatic artifact retention keeps its existing provenance and private-temp-path checks.

Shell support grants no new delegation authority. Continue using the current pi-subagents contract for authorized parallel/background children, worktrees, and explicit follow-ups. Package `/spawn`, structured spawn, or shell-launched agent CLIs are not alternate delegation routes. This is guidance, not a sandbox.

Before applying, let existing bg tasks finish or ask the user whether to stop them; do not migrate IDs or kill live user tasks. The user applies Home Manager with Pi closed, then starts a fresh session. Roll back only this shell package/configuration, guidance, and bg UI change if needed, preserving other migrations.

Run the shell integration with an isolated installation of exactly 0.18.0 and its dependencies:

```bash
PI_INTERACTIVE_SHELL_PACKAGE_DIR=/path/to/pi-interactive-shell \
  bash modules/pi/tests/run.sh interactive-shell.test.ts extension-loading.test.ts
```

The Linux test uses real PTYs, disposable HOME/agent/project directories, package-generated notifications, and native overlay input/rendering, without a model or external semantic service. It checks quiet dispatch through actual success/failure exits, input, takeover/attach/stop, reload and replacement cleanup, native bash results, tool cards, and artifact boundaries. The test explicitly skips without the package prerequisite; a skip is not migration completion. macOS and user Home Manager application require separate verification.

## Lens: diagnostics, navigation, and AST

`npm:pi-lens@4.4.12` replaces patched pi-lsp, the local semantic-refactoring implementation and the local AST extension. It loads once through the package manifest; bundled skills and prompts are filtered out. Use `pi_lens_activate_tools` when the package's situational navigation/AST tools are inactive. Nix still supplies all server binaries through `modules/lsp/default.nix` and retains `pkgs.ast-grep`. No server package, Pi version, or other extension dependency is changed.

### Policy and configuration ownership

`modules/pi/files/lens.json` is linked to the canonical machine-global `~/.pi-lens/config.json`, with server configuration under `lsp`. The old agent-directory LSP configuration is no longer installed. Lens owns connections, root discovery, diagnostics, tool schemas, and rendering; there is no compatibility wrapper for old names or preview tokens.

The Lens diagnostics widget starts hidden in every session. Use `/lens-widget-toggle` to show its compact summary below the editor or hide it again for that session. Diagnostics tools and the separate LSP footer status are unaffected. Run `/reload` in an open session to pick up the checkout-linked configuration.

The module exports `PI_LENS_DISABLE_LSP_INSTALL=1` and `PI_LENS_DISABLE_TOOL_INSTALL=1`. These stop Lens runtime installs, not Pi's npm package installation or npm dependency lifecycle scripts. The exact release contains prebuilt JavaScript and bundled grammars; its dependencies include ast-grep CLI/N-API native platform packages. The registry install does not run Lens's Git-source `prepare` build/grammar download. The inspected dependency postinstall copies the packaged ast-grep platform binaries. The isolated Linux install/self-test exercised that script and native binary, not a global install.

Automatic format, autofix, warning autofix, tests, startup scans, auxiliary scanners and unrelated analysis tools are off. Non-LSP dispatch runners are explicitly denied, leaving the LSP runner; startup is minimal. The list is tied to 4.4.12 and must be audited on a Lens upgrade. Lens's new read-before-edit monitor is not enabled as an additional permission policy; ordinary Pi approval and project trust remain in force. Lens may still emit its own advisory notices. Diagnostics and explicit tools are not a substitute for repository checks.

**Machine defaults are not enforcement.** Nearest project settings can override `format.enabled`, `autofix.enabled`, and `actionableWarnings.autoFix.enabled` in both directions. Before adopting Lens in a project, inspect that project's scoped configuration and resolve conflicts before editing. No private projects were scanned during this migration. Explicit `--no-autoformat --no-autofix` flags win over project mutation settings; they do not disable semantic rename. Do not enable autofix to regain AST apply without separate approval. `tests.enabled` and startup controls are global-only runtime controls. `effective_config` reports provenance, not values or proof of runtime enforcement (4.4.12 can report a project source even for a global-only input). Project trust gates launcher overrides; it is not a sandbox.

### Diagnostics and routes

Successful edit/write calls receive Lens feedback. For an explicit fresh file query use `lens_diagnostics({ path, source: "lsp" })`. A missing, silent, or timed-out server is unavailable/unconfirmed, **not clean**, even when an upstream tool result has `isError: false`. Review its text and details. Navigation uses `lsp_navigation({ operation, path, line, character })`; lines and character offsets are **one-based**. Lens subtracts one for LSP coordinates; for these UTF-16 servers, character offsets count UTF-16 code units, not bytes. Use the loaded schema rather than the removed tools' zero-based parameters.

| Former route | Lens catalog / managed difference |
| --- | --- |
| Nix | `nix` / nixd; catalog seeks flake.nix, unlike the old flake-or-Git markers |
| Go | `go` / gopls; workspace-priority go.work, go.mod/go.sum, then Git |
| TS/JS, including JSX/TSX and module suffixes | `typescript`; launcher override uses Nix's typescript-language-server wrapper and its bundled tsserver instead of Lens's project/managed tsserver discovery |
| Python and stubs | `python`; launcher override is `ty server`, with Jedi fallback denied; Pyright remains installed for other consumers |
| JSON/JSONC, CSS/SCSS/Less, HTML/HTM | `json`, `css`, `html`; existing vscode servers, catalog package/config roots with fallback |
| Lua | `lua` / lua-language-server; catalog roots |
| Shell | `bash` handles sh/bash; zsh now routes to `shuck`, **not supplied here**, rather than incorrectly treating zsh as Bash |
| Terraform / tfvars | `terraform` / terraform-ls; Terraform lock/directory markers, catalog fallback |
| YAML | `yaml` / yaml-language-server; yamllint/pyproject then Git roots |
| Markdown | `marksman`; md (and upstream mdx) supported; old `.markdown` suffix has **no catalog route** |
| TOML | `toml` / taplo; pyproject/Cargo/taplo then Git roots |
| Rust | `rust` / rust-analyzer from the active toolchain; needs Cargo.toml, not merely Git or a detached file |

The remaining built-in catalog stays upstream-owned; this is not an allowlist of all languages Lens can recognize. Root, dialect, exclusion and timing policies are not identical to the removed per-server configuration. No new language-server binary is installed to fill the zsh or other gaps.

### Semantic rename and code actions

Use `lsp_navigation` with `operation: "rename"`, `newName` and `apply: false` for an explicitly requested rename. Its default text preview summarizes affected files and edit counts; `view: "json"` exposes the edit envelope for review. Only after review and authorization use `apply: true`. Apply requests the server **again**: there is no single-use preview ID, complete stored-diff guarantee, or byte-fixed preview transaction. Re-preview after intervening changes. The isolated TypeScript test confirmed that a change between preview and apply is retained and apply succeeds; a concurrent change during the apply request is refused as stale before mutation. This is narrower than the old snapshot contract, not equivalent protection.

`operation: "codeAction"` lists actions. It does not reproduce the removed select/resolve/preview-ID/apply workflow, and `apply: true` does not turn that operation into an action executor. Apply approved changes through ordinary edits or an editor. Upstream `executeCommand` and `rename_file` existence does not grant permission to run server commands or move files; those require separate authorization.

### AST contract and functional reductions

Use `ast_grep_search`, `ast_grep_outline`, and `ast_grep_replace` with explicit, bounded `paths`. Replacement takes `pattern`, `rewrite`, `lang`, and `apply: false`. It matches syntax, not symbol identity. Unlike the removed explicit-file-only API, Lens also accepts directory/glob scopes, so bound the request deliberately.

**AST apply is disabled in this initial migration.** 4.4.12 refuses `ast_grep_replace({ ..., apply: true })` under `autofix.enabled: false`, even after a preview. Its preview's generic suggestion to use apply does not override policy. Use approved ordinary edits or an editor instead. The old `ast_apply`/`ast_discard`, single-use token, exact stored bytes and session-local snapshot guarantees are gone. Do not add a compatibility layer or patch Lens to restore them.

### Applying and verifying

Close Pi, apply Home Manager yourself, refresh the shell environment and start a new Pi session. Do not deploy old LSP/AST and Lens together. Existing unmanaged Lens config can conflict with the new managed link: review/back up it before switching; do not overwrite or delete user state automatically. `/reload` cannot install a changed package selection, config link or environment. Rollback restores this migration's removed sources and package/config declarations from Git without reverting independent question, shell or prompt changes. No Home Manager switch or live-server/cache cleanup was performed by the migration tests.

## Task, analysis, and artifact extensions

These extensions have independent entry points and do not replace Pi's built-in tools or its JavaScript `codemode`. Pi loads rpiv-todo from the selected npm package; Home Manager installs the local Python and artifact extensions and their required binaries. Apply Home Manager, then restart Pi; `/reload` alone does not change the managed package selection or extension links.

| Extension | Model tools | Interactive commands | Purpose |
| --- | --- | --- | --- |
| `@juicesharp/rpiv-todo` | `todo` | `/todos` | Task tracking with dependencies and current-branch restoration |
| `modules/pi/files/extensions/python-kernel/` | `python_cell` | `/python status`, `/python reset` | Persistent Python cells, captured output, and images |
| `modules/pi/files/extensions/session-artifacts/` | `artifact` | `/artifacts` | Durable text snapshots with listing, paging, and search |

Python execution has the same filesystem and network permissions as Pi; it is not a sandbox. Lens AST policy is described above. Artifact storage is local and can retain sensitive command output; register only material appropriate to keep with the session.

### Tasks

`todo` accepts `create`, `update`, `list`, `get`, `delete`, and `clear`. Tasks have a numeric ID, `subject`, optional `description`, `activeForm`, `owner`, `metadata`, and `blockedBy` dependencies. The states are `pending`, `in_progress`, `completed`, and `deleted`; deletion retains a tombstone. Missing dependencies and cycles are rejected. Dependencies record relationships, not permission to launch work or an automatic scheduler. Update statuses explicitly: completing a task does not start the next one, and the package's single-active-task guidance is not a state-machine constraint.

`/todos` shows tasks grouped by status. A widget above the editor shows progress; `Ctrl+Shift+T` toggles collapse. Completed rows disappear from the widget on the next agent turn but remain in task state and `/todos`. Defaults allow 12 content rows plus a spacer; optional `maxWidgetLines` and `collapseKey` settings live in the package-owned XDG config file. This module installs no config override. Each mutation appends a snapshot to the current session branch, including calls through codemode. Session lifecycle handlers replay that branch after reload, resume, tree navigation and compaction. The package makes no model calls.

RPIV uses its own task list without a phase hierarchy. Historical session entries from the former `phase-tasks` extension remain unchanged and are not converted into RPIV tasks. This module provides `todo` and `/todos`, not the former `task_list` tool or `/tasks` command.

### Python cells

`python_cell` accepts `code`, optional integer `timeout` (1–120 seconds, default 30), and `reset`. A lazy `python3` subprocess preserves variables, functions, future imports, and top-level `await` across serialized cells. Final expressions, stdout, stderr, and tracebacks are returned. `display_image` accepts PNG/JPEG bytes, a file path, or a matplotlib figure when matplotlib is already installed; libraries are not installed automatically.

Python exceptions fail the tool while preserving earlier mutations. Timeout, cancellation, process failure, reset, session replacement, and tree navigation discard the namespace; interrupted code is never replayed. Resume and reload start fresh rather than reconstructing historical Python state. Text output and images are bounded; discarded excess text is not an artifact. Python can allocate memory, access files/network, and launch processes with the user's privileges. Its file writes do not participate in Pi's mutation queues.

### Retained text artifacts

`artifact` provides `register`, `list`, `read`, and literal `search`. Registration takes an explicit `path` and optional title/line range; later reads use the registered ID. Paging returns `nextOffset` and `nextColumn` so long Unicode lines can be continued. `/artifacts` opens the selector and read-only content viewer; `/artifacts register PATH`, `read ID [OFFSET]`, `search ID TEXT`, and `help` also work. The registration command treats its remaining argument as the path, without shell quoting.

Snapshots are private local copies under Pi's agent directory in `session-artifacts/<origin-session-hash>/`. They survive source deletion and shutdown. Branch-local index entries restore on resume and fork; forks can still reference the originating session's files. No automatic deletion occurs, including when a session is deleted. Close Pi before manually removing unneeded owner directories, and check fork references first. Limits are 2 MiB per source, 20,000 snapshot lines, and 64 files/32 MiB per originating session. Cancelled registration can leave an unindexed retained snapshot that counts toward this quota.

Automatic retention recognizes Pi 1.5.2's native `bash` and `codemode` full-output metadata only after checking builtin provenance and private temporary-file paths. Interactive-shell logs are external and require explicit registration. Free-text paths are never scraped, and other directories are not scanned.

Run the focused feature checks with Python on PATH:

```bash
PI_RPIV_TODO_PACKAGE_DIR=/path/to/isolated/rpiv-todo \
  bash modules/pi/tests/run.sh rpiv-todo.test.ts python-kernel.test.ts session-artifacts.test.ts
```

The RPIV test requires the exact selected package with its runtime dependencies; it performs no downloads and is skipped when `PI_RPIV_TODO_PACKAGE_DIR` is absent. It exercises the actual Node/jiti-loaded tool and widget with Pi's in-memory session manager: mutations, cycle rejection, replay hooks, session isolation, collapse and width bounds. It does not establish a live model turn or full TUI interaction.

For an interactive smoke check, use a disposable directory: create and update todos, inspect `/todos` and collapse the widget at narrow width, evaluate a variable across two Python cells, register a log and read it after deleting its source. `/python reset` must clear the variable.

The module uses [Pi's built-in MCP client](https://pi.dev/docs/latest/mcp) for local stdio and remote streamable HTTP servers. No adapter package is required.

Home Manager uses `pi mcp add` to add `nixos` and `context7` only if their names are missing. On the first switch from the old checkout symlink, activation copies its contents into a private regular file before adding servers. Keep `modules/pi/files/mcp.json` available for devices that have not switched yet. Existing entries, including Plane and device-specific settings, are preserved. Fresh devices do not get Plane automatically.

Use `pi mcp add` on the target device to add a server or replace its entire definition under the same name. For example:

```bash
pi mcp add docs --url https://example.com/mcp
pi mcp remove docs
```

Removing a shared server makes activation add it again on the next switch. Disable it in `/mcp` instead if you want it to stay off. Plane is opt-in. On a device that needs it, add this entry to the `mcpServers` object in `~/.pi/agent/mcp.json`. Pi's add command does not expose the OAuth metadata URL and scope.

```json
{
  "plane": {
    "url": "https://plane-mcp.teamdev.ai/mcp",
    "description": "Read and update work items, projects, states, and links in the devsisters Plane workspace (plane.teamdev.ai)",
    "oauth": {
      "clientId": "plane",
      "callbackPort": 8181,
      "authServerMetadataUrl": "https://auth.devsisters.cloud/auth/realms/devsisters/.well-known/openid-configuration",
      "scope": "openid email profile offline_access"
    }
  }
}
```

Pi connects enabled servers in the background at startup. With the default `codemode` exposure, the model discovers and calls tools on demand without loading every schema into the prompt. Discovery is on demand; the connections are not.

Use `/mcp` to inspect connections in a session or `pi mcp list` to check connections and tool discovery from the shell. To verify that a service can fulfill requests, run a read-only tool call through `codemode`. A server can list its tools even when a request fails because of a service quota or missing permissions.

Run `/reload` in an open Pi session after changing its MCP configuration.

Cloudflare operations use the `cf` CLI installed by the common Home Manager profile. See [Cloudflare CLI setup](../../README.md#cloudflare-cli) for authentication and command discovery.

## jari workers

The shared jari profile enables agenix provisioning for Hindsight by default. The `jari-amber` host key is enrolled; no local enable setting is needed. On activation, agenix uses the worker's SSH host Ed25519 identity to install `/run/secrets/hindsight-client-token` for `jari` with mode `0400`. Before applying the profile to a worker whose host key is not enrolled, either set `modules.jari.hindsightAgenix.enable = false;` in its NixOS configuration or enroll the host public key for the Hindsight client token and re-encrypt that secret. Otherwise agenix cannot decrypt it. Workers that opt out can still supply the same runtime file. Do not put the token in the checkout, Nix configuration, or shell profile. Verify the private route with `curl -fsS https://pylv-sepia.tailbbb9bf.ts.net/health/ready`, then verify authentication as `jari` with `hindsight bank list`.

To also enable Pi session retention on that worker, run the following as `jari` after provisioning the token. This enables memory for work repositories as well. The link command will not replace an existing file; review any existing configuration before proceeding. Restart Pi in the worker checkout and run `hindsight_diagnose` to confirm the tailnet endpoint and `config-flakes` bank. Leave the preset unlinked on workers that should not retain sessions.

```sh
mkdir -p ~/.hindsight
ln -s ~/.config/hindsight/jari-pi.json ~/.hindsight/coding-agent.json
```

The `devsisters-jari` profile installs everything above except `pkgs.pi`. jari runs its own pinned Pi from `/opt/jari-cli` in terminal and chat tabs and leaves `~/.pi` to this module. Because that Pi is built without [the local package patches](../../packages/apps/pi/patches.nix), Herdr file links and image paste labels are absent, and rounded tools leave extension messages unframed. The module sets `PI_UNPATCHED=1` there, so rounded tools skip their notice to apply Home Manager, which cannot add the frames.

jari starts Pi from its worker service with a fixed `PATH` and environment that never reads the shell profile, so `mcp-nixos`, the Lens language servers, and the Lens install switches in `home.sessionVariables` would be missing. With `modules.pi.launchEnvironment.enable`, the module installs a generated `~/.pi/agent/extensions/launch-environment.ts`, which sets those session variables in Pi's process and moves the `pathHead` directories (jari's directories, then the Nix profiles, matching the shell's order) to the front of its `PATH`. MCP servers, Lens, the `bash` tool, and subagents started from Pi inherit the result.

jari's Pi accepts the worker's Codex login only for the legacy `openai-codex` provider. The common settings and subagent model scope use `openai`, so run `/login openai` once in a jari Pi tab as described in [ChatGPT authentication](#chatgpt-authentication); until then Pi falls back to an authenticated `openai-codex` model and subagents fail the model scope.

## Theme

Pi selects the generated `one-half-light` theme from `themes/exports/pi/one-half-light.json`. The built-in `system` theme remains available in `/settings` to derive colors from the terminal's background and ANSI palette.

The local prompt derives path and model badges from the active Pi theme: the path uses an accent-tinted background with a rounded left edge and an arrow-shaped right edge, while the model uses a label-tinted background with rounded edges. Both use stronger foreground colors for readability. Git counts retain their symbols and use warning (`*`), success (`+`), and accent (`?`) colors; query failures use error. Thinking follows the selected level's theme color, fast mode uses the keyword color in bold, and filled quota cells use success. Context usage uses accent below 70%, warning from 70%, and error from 90%. The input marker also uses accent. Narrow layouts drop the model badge before truncating its text. No shared theme values or generated exports are changed. For an already installed checkout-linked prompt extension, `/reload` picks up rendering changes.

## Verification

The test runner requires Nix, Bun, Node.js, jq, ripgrep, and an installed Pi matching the version in `packages/apps/pi/package.nix`. It locates the Nix-installed runtime through `pi` on PATH; for another installation layout, set `PI_TEST_NODE_MODULES` to that runtime's node_modules directory. It evaluates `modules/pi/tests/module-config.nix` with the root flake's pinned Nixpkgs and Home Manager libraries to obtain the generated common settings, packages, and extension links for both Herdr conditions and a jari profile, without building packages. It copies the module to a temporary directory, links the installed dependencies there, and uses a temporary Pi agent directory. It does not install dependencies or change the checkout, credentials, or live settings. Pass test filenames to run a subset:

```bash
bash modules/pi/tests/run.sh module-config.test.ts subagent-profiles.test.ts
```

Set `PI_RPIV_QUESTION_PACKAGE_DIR` to an isolated `@juicesharp/rpiv-ask-user-question@2.15.0` package with its runtime dependencies to enable the questionnaire loader check in `extension-loading.test.ts`. It checks the monorepo package metadata, exact configured identity, single tool registration and fresh rediscovery. With all migration package directories supplied, the coexistence check includes the questionnaire too. An unset variable skips this check; loading is not evidence of live questionnaire or overlay focus behavior.

Prompt UI checks use the installed Pi components without a Powerline package dependency:

```bash
bash modules/pi/tests/run.sh module-config.test.ts extension-loading.test.ts prompt-ui.test.ts prompt-statusline.test.ts prompt-git-status.test.ts codex-fast-mode.test.ts subscription-usage.test.ts
```

The prompt mouse regression must pass: clicking the displayed character must select that character, not a column shifted by the prompt prefix. Node/jiti discovery is checked separately from headless component rendering/input.

When `PI_INTERACTIVE_SHELL_PACKAGE_DIR` and `PI_LENS_PACKAGE_DIR` are supplied, the loader suite also checks their coexistence with the local prompt UI, status providers, and rounded-tools without duplicate tools or commands. Supplying `PI_RPIV_QUESTION_PACKAGE_DIR` includes the questionnaire; `PI_RPIV_TODO_PACKAGE_DIR` includes the task package and checks that no legacy `task_list` or `/tasks` is registered. This is loading evidence, not simultaneous live overlay/focus verification.

Lens integration requires an already installed exact 4.4.12 package with its dependencies; it never downloads them or reads the live agent package directory implicitly:

```bash
PI_LENS_PACKAGE_DIR=/path/to/isolated/node_modules/pi-lens \
  bash modules/pi/tests/run.sh lens.test.ts
```

Without that variable the Lens test is explicitly skipped, **not verified**. It uses generated package settings with unrelated packages omitted, real Pi Node/jiti loading and SDK reload/new-session/shutdown, disposable projects and isolated HOME/agent/Lens/cache locations. Registered tool execution and Pi's real hook dispatcher are driven without a model; this is not a live model-turn test. It exercises native AST, project overrides, missing/unresponsive servers, scoped route reconciliation, real available servers, rename conflicts, expansion and process teardown. Empty navigation answers and unavailable language binaries are printed as `UNVERIFIED`, not counted as successful feature witnesses.

After changing this module:

1. Validate edited JSON files with `jq -e . <file>`.
2. Run `bash modules/pi/tests/run.sh`, `bash modules/pi/tests/sync-settings.sh`, and `bash modules/pi/tests/seed-mcp.sh` for extension and device-local configuration checks. Run `nixfmt modules/pi/default.nix` when the Nix module changes.
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

6. In Pi, edit a supported disposable file and confirm that its LSP diagnostics are appended to the tool result. Also verify `lsp_navigation` hover or documentSymbol on that file.
7. Verify `/fast status`, MCP discovery, structured questions, and the
   web-access tools relevant to the change.
8. In a fresh session, verify `web_enable` and `subagents_enable`, then activate each and confirm that the corresponding tools appear. Verify `/mcp`, `pi mcp list`, and a read-only MCP call through `codemode`, with no extension-loading warnings.

For the subagent rollout, also run `/subagents-doctor` and `/subagents-models`. Inspect `/subagents researcher prompt` and `/subagents reviewer prompt` to confirm the local definitions, then verify one foreground `scout`, one `researcher`, and a read-only `reviewer` before trying `worker` against a disposable fixture. Researcher must activate the web tools, search, fetch an original source, and return citations without creating repository notes. Reviewer must inspect a disposable diff and run a known side-effect-free check, report its actual outcome, and leave files and the index unchanged. These single-child checks should not create an additional Git worktree. To validate the expanded execution policy, use a disposable repository and explicitly authorized parallel workers in separate worktrees, then check a background completion and a review/feedback follow-up. Verify the actual model/thinking, edit isolation, child results, and process termination; do not use live user work as a fixture. A passed settings, scheduler, or frontmatter check alone does not establish child tool loading or model behavior.

Do not commit credentials, OAuth tokens, API keys, `auth.json`, session files,
or MCP credential caches. Pi has no built-in sandbox; packages, extensions,
skills, and tool commands run with the current user's permissions.
