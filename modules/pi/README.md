# Pi coding agent module

This module manages the global Pi coding-agent setup used by this repository.
It installs Pi and the NixOS MCP server, then exposes tracked configuration
under `~/.pi/agent/` through generated files and Home Manager out-of-store
symlinks.

## Managed resources

| Repository path | Runtime path | Purpose |
| --- | --- | --- |
| `files/settings.json` | `~/.pi/agent/settings.json` | Common settings merged into a device-local writable file |
| `files/web-search.json` | `~/.pi/web-search.json` | Web Access defaults |
| `files/lsp.json` | `~/.pi/agent/lsp.json` | LSP server routes and diagnostics settings |
| `files/mcp.json` | `~/.pi/agent/mcp.json` | Native MCP server configuration |
| `agent-core/rules/` and `agent-core/adapters/pi.md` | `~/.pi/agent/AGENTS.md` | Generated shared and Pi-specific instructions |
| `agent-core/rules/OPERATING.md` | `~/.pi/agent/APPEND_SYSTEM.md` | Operating invariants added to Pi's system prompt |
| `files/extensions/` | `~/.pi/agent/extensions/` | Local Pi extensions |
| `agents/researcher.md` and `agents/reviewer.md` | `~/.pi/agent/agents/` | Foreground specialist prompts and tool contracts |
| `themes/exports/pi/one-half-light.json` | `~/.pi/agent/themes/one-half-light.json` | Generated One Half Light theme |
| `files/themes/claude-like.json` | `~/.pi/agent/themes/claude-like.json` | Alternate dark theme |
| Agent core skill sources | `~/.agents/skills/` | Codex/Pi shared pool, installed or updated by shared activation |

The module also installs:

- `pkgs.pi`
- `pkgs.mcp-nixos`

`settings.json` is a private, writable regular file on each machine, not a checkout symlink. Home Manager merges the common settings from `files/settings.json` into it before linking the new generation; common top-level keys take precedence, while other local keys are preserved. Pi creates `deviceId` on first ChatGPT login and keeps it, `trackingId`, and `lastChangelogVersion` local. The sync rejects these fields in common settings. Existing symlink installations are migrated without writing their local state back to the checkout.

`/settings` and package commands modify only the local settings file. Changes to keys managed by the common file last until the next Home Manager application; edit the repository file to make them common defaults. Close Pi before applying Home Manager to avoid concurrent settings writes. Other mutable configuration still uses out-of-store symlinks. Generated instructions and common settings change only after applying Home Manager.

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

### Automatic compaction

`compaction.modelOverrides` configures `openai/gpt-6-astra` to reserve 54,400 tokens and retain roughly 40,000 recent tokens without summarization. With Pi's registered 272,000-token context window, automatic compaction starts above 217,600 tokens (80%). Other models use Pi's default compaction budgets. The reserve is an absolute token count, so review it if the model's registered context window changes.

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

The package list is intentionally version-pinned:

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

`pi-subagents` loads only its extension; its bundled skills and prompt templates are filtered out so they cannot opt into background workflows or wide fanout. The current rollout uses packaged agents plus local `researcher` and `reviewer` definitions with explicit Luna/Sol/Astra routing. Delegation defaults to foreground, stores artifacts under the parent Pi session, allows eight child launches per parent session, and blocks nested delegation at child depth. Automatic missions, schedules, resumption of interrupted background workflows, and the generic `delegate` agent are disabled. Tool activation stays dynamic through `subagents_enable`.

Delegation defaults to `openai/gpt-6.1-sol`; `scout` and `researcher` use `openai/gpt-6-luna`, `planner` uses Sol, and `oracle` uses `openai/gpt-6-astra`. All three models are in the allowlist. The `reviewer` uses `high` thinking.

The packaged `planner`, `worker`, and `oracle` fork defaults are overridden to fresh context. The local researcher and reviewer also use fresh context. Use an explicit `context: "fork"` only when the child genuinely needs the parent transcript; otherwise pass a compact task contract. The researcher skips inherited project instructions because its role prompt and research task provide its operating context. Other agents retain repository instruction inheritance, but fresh children exclude the operator's global context and skills catalog unless explicitly configured otherwise. The child runtime also excludes the operator's `APPEND_SYSTEM.md`; a parent must pass relevant constraints in the task contract rather than assume its whole prompt is inherited. When a child persists a substantial artifact, prefer `outputMode: "file-only"` so the full result is not injected back into the parent context.

Only `worker` retains normal implementation tools. `scout`, `researcher`,
`context-builder`, and `reviewer` have no `edit` or `write` tool. Their `bash`
access is for inspection and verification and is not a security boundary; Pi
packages and child processes still run with the current user's permissions.
For the initial rollout, use one direct foreground child at a time and do not request `workflowScript`, background runs, or worktrees. The configured global concurrency limit applies to both legacy multi-child paths and `workflowScript` children.

### Specialist contracts

`agents/researcher.md` loads the already pinned `pi-web-access` package through `subagentOnlyExtensions`. Foreground children do not load ambient extensions, and listing a tool name alone does not load its provider. The path assumes Pi's managed npm package location under `~/.pi/agent/npm/node_modules/`; review it when changing package installation layout. The role includes `web_enable`, the search/fetch tools, `source_check`, and native `contact_supervisor`. Web tools activate on demand. It has no shell or file-writing tools, does not request progress files or a default repository output, and returns evidence and gaps to the parent. An explicitly requested runtime artifact is still managed by pi-subagents.

`agents/reviewer.md` replaces the bundled reviewer prompt, which prohibits shell commands despite this module's inspection-oriented `bash` selection. The local role permits read-only Git inspection and focused local checks authorized by the task or repository instructions after their side effects are checked. It prohibits source/snapshot rewrites, auto-fixes, installation, staging, commits, branch changes, production access, and external writes. Unknown or unauthorized checks are reported to the parent rather than run. This is an instruction contract, not a shell sandbox; `acceptanceRole: "read-only"` does not restrict shell execution.

The local files own role prompts, context inheritance, tool selection, and child extension loading. `files/settings.json` owns model and thinking overrides. Avoid adding a settings override that silently replaces those role fields.

### Prompt baseline and verification evidence

The 2026-11-02 baseline used Pi 1.4.0, pi-subagents 0.84.0, and pi-web-access 0.46.0. The installed agent definitions were resolved with the package's discovery code. Foreground child requests were captured after prompt hooks using the installed child-session factory and a local response stub, without sending the captured prompts to a provider. The parent instruction sources were inspected in the active session. Raw request captures remain outside the repository because they contain local paths and instructions.

| Role | Baseline prompt and context | Baseline issue | Managed contract |
| --- | --- | --- | --- |
| Astra parent, high | Pi base prompt, operating addendum, global/project instructions, skill catalog, active extension guidance | External research skill requires background execution and repository notes | Pi adapter retains foreground-only rollout; canonical research skill follows runtime policy and requested output |
| Sol worker, medium | Packaged replacement prompt, repository context; no global context, operating addendum, or skill catalog in fresh requests | Parent instruction inheritance must not be assumed | Unchanged; parent supplies task-specific constraints |
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

[Discovery and task-contract cases](../../agent-core/skills/parallel-research-merge/references/evaluation.md) define expected routing and stopping behavior. Renderer and source checks verify packaging and instruction placement, not automatic selection, model compliance, or an improvement in time or tokens. Apply Home Manager to refresh generated instructions and shared skill copies together, then start a fresh Pi session. Editing the canonical sources alone does not update an already loaded prompt.

## Local extensions

### Herdr parent status

When both Pi and Herdr modules are enabled, Home Manager installs `herdr-agent-state.ts` from the pinned Herdr source into `~/.pi/agent/extensions/`. This official integration reports the parent TUI session's working, idle, and blocked states. `agent_start` marks it working; `agent_settled` marks it idle only after automatic retries, compaction, and queued continuations finish. Headless child sessions do not report parent state.

Apply Home Manager and run `/reload` in Pi, or start a new Pi session inside Herdr, to load the integration. Home Manager owns this file; update it through the Herdr package pin rather than `herdr integration install pi`. The display metadata extension below does not report lifecycle state.

### Herdr subagent sidebar

`files/extensions/herdr-subagents.ts` reports the current parent session model and displays foreground `pi-subagents` children beneath their parent Pi entry in Herdr's expanded Agents sidebar. The parent row shows the agent name and model, updating when the selected model changes. Each child uses two lines: its agent name and resolved model, followed by its current tool and path, latest output, or assigned task. Both the `subagent` tool and `/run` foreground execution are supported. The layout hides pi-subagents' background summary; this extension does not enumerate background children.

The extension publishes display metadata only from the parent interactive Pi session inside Herdr. Completed, failed, interrupted, and detached foreground children disappear from the list. Session shutdown clears its tokens; active metadata expires after 45 seconds without refresh if Pi crashes. At most seven children fit in Herdr's 16-line layout. Larger groups show the first six children and an overflow count. Clicking any child line focuses the parent pane.

The progress payload and slash events follow `pi-subagents` 0.84.0. Review this integration when updating that package. It does not import the package's internal modules or modify its execution behavior. Herdr failures produce a warning and leave Pi running.

Home Manager installs the extension. After applying the configuration, run `/reload` in Pi and `herdr server reload-config` for the [Herdr sidebar layout](../herdr/README.md#subagents). For a foreground smoke test, run `/run scout List the top-level files without changing anything` inside a Herdr Pi pane and verify that the child appears, updates, and disappears when finished. Run `bun test modules/pi/tests/herdr-subagents.test.ts` for the isolated event and metadata tests.

### OpenAI fast mode

`files/extensions/codex-fast-mode.ts` provides:

- `/fast [on|off|status]`
- `service_tier: "priority"` for `openai` requests while enabled
- fast-mode status for the prompt's integrated statusline

Fast mode starts disabled in each new session; use `/fast on` to enable it. Toggle changes are stored in the session so resumed branches recover their previous state. The extension owns the priority-mode request field and publishes its display state through `codex-fast-mode`. Its statusline renderer supplies the editor extension with working directory, Git branch, model, thinking level, fast-mode state, context usage and extension statuses. The statusline does not display or aggregate cumulative input/output tokens.

Subagent child processes are detected through `PI_SUBAGENT_CHILD=1` and do
not receive the priority service tier. `PI_SUBAGENT_PARENT_SESSION` is also set
in the parent UI session for permission forwarding, so it must not be used as
the child-process signal. Parent Pi sessions continue to use fast mode normally.

### Codex subscription usage

`files/extensions/codex-usage.ts` adds `/codex-usage` and shows the Codex weekly quota remaining and reset date and time in the computer's time zone immediately to the left of the model in the prompt's top statusline. The display starts with a ten-cell remaining-quota bar, such as `###------- weekly 33% left`, followed by a timestamp such as `reset 2026-11-05 03:00`; each `#` represents a full ten percentage points remaining. Narrow terminals truncate the usage text or omit it to preserve context, model, and task-selection information. Interactive parent sessions fetch usage at startup and every minute without blocking startup. `/codex-usage` refreshes immediately and shows the local reset date and time without a time-zone suffix, along with the source timestamp. Concurrent refreshes share one request, each request has a 30-second timeout, and shutdown or reload cancels pending work. Non-interactive sessions and subagent processes do not poll.

The extension runs `codexbar usage --provider codex --source oauth --json --no-credits` using the CodexBar CLI already installed by the common profile. It reads the weekly `usage.secondary` window, not session token totals or API credit balances. OAuth uses the Codex CLI account selected by CodexBar and `CODEX_HOME`, which can differ from Pi's ChatGPT login. Use the same account in both clients to track the subscription used by Pi. The extension does not read, copy, or manage authentication files; CodexBar owns usage retrieval and its authentication behavior. It does not fall back to browser cookie extraction.

Failed queries replace the displayed quota with an unavailable status; `/codex-usage` reports a bounded error without raw CLI output or account identifiers. Data older than ten minutes is marked stale. Missing reset times remain unknown, and an elapsed reset is marked as awaiting fresh usage rather than assuming a replenished quota. Usage stays in memory and is not added to the model context or session history.

Apply Home Manager and restart Pi to install the extension. To try it before applying Home Manager, start `pi --extension ./modules/pi/files/extensions/codex-usage.ts` from the repository root. If a fetch fails, check that `codexbar` is on `PATH` and that the Codex CLI account is authenticated; Pi's `/login openai` does not authenticate CodexBar.

### Prompt and cursor rendering

`modules/pi/files/extensions/hardware-cursor-only.ts` owns the editor and the empty footer. It uses the renderer from `modules/pi/files/extensions/codex-fast-mode.ts` to place status information in the editor's top border. The working directory and branch share a subtle theme-backed background with a rounded Powerline left cap and an arrow-shaped right cap, followed by the context meter. The caps use the same color as the segment background and require a font with Powerline glyphs, such as a Nerd Font. Background tasks, other extension statuses, weekly quota, and the model appear on the right, connected by a thin rule. At narrower widths, the renderer shortens the path and quota, then omits secondary statuses and the location before shortening essential context and model information. A selected `/bg` item retains a visible marker and hardware cursor even in a very narrow pane.

The first visible input row starts with `╰─` instead of a boxed input area. Built-in editor padding reserves space for the prompt, so the marker is not part of submitted text. At three or four columns it uses only `╰`; below three columns it omits the marker. The editor keeps Pi's wrapping, history, autocomplete, and mouse positioning. Hidden input lines retain scroll hints. The bottom border is omitted when no lines are hidden below; autocomplete retains a blank separator so its mouse coordinates remain unchanged.

The extension removes Pi's software cursor markers while preserving the terminal cursor used for IME positioning. It is paired with `showHardwareCursor: true` in `modules/pi/files/settings.json`. Review it when updating Pi if cursor rendering or IME positioning changes. With the existing checkout-backed extensions installed, `/reload` applies these changes without a Home Manager switch. Check a narrow pane, multiline Korean input, autocomplete, and Down/Enter task selection after reloading.

### Terminal title on exit

Pi sets the terminal title to `π - <directory>` and does not clear it on exit, so terminal applications can keep showing it after returning to the shell. `files/extensions/terminal-title-reset.ts` sends an empty title when the TUI process exits after a quit. It writes from a process `exit` listener because Pi 1.5 rewrites the title after `session_shutdown`. Session reload, new, resume, and fork leave the title to Pi.

## On-demand tools

Fresh sessions use the packages' own `web_enable` and `subagents_enable` tools to activate the full web and delegation tools when needed. Package commands such as `/run` remain available. Native MCP uses Pi's `codemode` tool without declaring every server tool to the model. The module does not install a custom `enable_tools` tool or `/tool-profile` command.

When migrating an existing installation, apply Home Manager to remove its managed `~/.pi/agent/extensions/tool-profiles` symlink, then restart Pi. New sessions use the default activation state; resumed sessions may retain tools already activated in their transcript.

## Background shell tasks

`pi-bg-bash` replaces Pi's model-facing `bash` tool, not the system Bash executable, login shell, or user-entered `!` commands. Ordinary calls still wait for exit; `background: true` returns a task ID immediately, and completion sends the agent the exit state and recent output. `Ctrl+Shift+B` moves running foreground tool calls to the background when the terminal supports modified keys. Elapsed time alone does not trigger that transition.

Use `/bg` to view task state, runtime, and commands. The prompt's top statusline shows `/bg (N)` at the start of its right-hand status group when the command is available. `N` is the number of running tasks reported by pi-bg-bash's status, excluding finished tasks; it shows zero when none are running. From an empty input with autocomplete closed, press Down to highlight `/bg (N)` in the terminal palette's purple, Enter to open `/bg`, or Up/Escape to return to the input. The editor publishes selection through the `bg-selection` status, and the statusline highlights it without adding an editor row or duplicating the package's `bg: N` status. Typing or pasting leaves the selection and edits the input normally. Nonempty input retains normal cursor and history navigation. In `/bg`, press Enter for live output, `x` to stop a task after confirmation, and Escape to go back. The agent can use `bash_tasks`, `bash_output`, `bash_kill`, and `bash_watch` to inspect or manage tasks. Output watches are optional; completion notifications do not require them.

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

After changing this module:

1. Validate edited JSON files with `jq -e . <file>`.
2. Run `bun test modules/pi/tests` and `bash modules/pi/tests/sync-settings.sh` for extension and device-local settings checks. Run `nixfmt modules/pi/default.nix` when the Nix module changes.
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

For the subagent rollout, also run `/subagents-doctor` and `/subagents-models`. Inspect `/subagents researcher prompt` and `/subagents reviewer prompt` to confirm the local definitions, then verify one foreground `scout`, one `researcher`, and a read-only `reviewer` before trying `worker` against a disposable fixture. Researcher must activate the web tools, search, fetch an original source, and return citations without creating repository notes. Reviewer must inspect a disposable diff and run a known side-effect-free check, report its actual outcome, and leave files and the index unchanged. Confirm that no `.pi-subagents/` directory or additional Git worktree appears in the repository. A passed settings or frontmatter check alone does not establish child tool loading or model behavior.

Do not commit credentials, OAuth tokens, API keys, `auth.json`, session files,
or MCP credential caches. Pi has no built-in sandbox; packages, extensions,
skills, and tool commands run with the current user's permissions.
