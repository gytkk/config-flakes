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
| `config/themes/exports/pi/one-half-light.json` | `~/.pi/agent/themes/one-half-light.json` | Generated One Half Light theme |
| `files/themes/claude-like.json` | `~/.pi/agent/themes/claude-like.json` | Alternate dark theme |
| Agent core skill sources | `~/.agents/skills/` | Codex/Pi shared pool, installed or updated by shared activation |

The module also installs:

- `pkgs.pi`
- `pkgs.mcp-nixos`

`settings.json` is a private, writable regular file on each machine, not a checkout symlink. Home Manager merges the common settings from `files/settings.json` into it before linking the new generation; common top-level keys take precedence, while other local keys are preserved. Pi creates `deviceId` on first ChatGPT login and keeps it, `trackingId`, and `lastChangelogVersion` local. The sync rejects these fields in common settings. Existing symlink installations are migrated without writing their local state back to the checkout.

`/settings` and package commands modify only the local settings file. Changes to keys managed by the common file last until the next Home Manager application; edit the repository file to make them common defaults. Close Pi before applying Home Manager to avoid concurrent settings writes. Other mutable configuration still uses out-of-store symlinks. Generated instructions and common settings change only after applying Home Manager.

## Main settings

`files/settings.json` currently selects:

- `openai/gpt-6.1-sol` with `high` thinking
- the generated `one-half-light` theme
- a startup header without the loaded-resource listing
- a fixed `❯` prompt with space before the input and default output padding
- the hardware terminal cursor for IME positioning
- prompt-cache notices, with cache warming left at Pi's default `"streaming"` mode

Pi uses the default fullscreen mode and built-in keybindings: `Home` and `Ctrl+A` move to the input line's start, while `Ctrl+Home` moves to the top of the fullscreen conversation. The module does not install a keybinding override.

### ChatGPT authentication

Run `/login openai` in Pi and choose `Sign in with ChatGPT`, then approve the browser login. Pi stores the OAuth credential under `openai` in `~/.pi/agent/auth.json`. Verify without printing credentials:

```bash
pi auth check --provider openai --json --no-refresh
```

The parent and delegated chat models use `openai`. In Pi 1.1, its GPT-6 Luna classifier is a separate model type and requires an API key; ChatGPT OAuth supports the chat model but not the Decisions API classifier. Use the existing `with-openai` wrapper in a separate process for API-key clients rather than replacing the subscription credential. See the [OpenAI API module](../openai-api/README.md).

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
| `pi-web-access` | `0.46.0` | Web search, source checks, and content fetching |
| `pi-subagents` | `0.84.0` | Foreground-first delegated Pi sessions |

Versioned npm packages are skipped by `pi update --extensions`. Update pins
through a reviewed repository change, and review package source and release
notes because Pi packages execute with the permissions of the Pi process.

`pi-subagents` loads only its extension; its bundled skills and prompt templates are filtered out so they cannot opt into background workflows or wide fanout. The initial rollout uses packaged agents with explicit Luna/Sol/Astra routing. Delegation defaults to foreground, stores artifacts under the parent Pi session, allows eight child launches per parent session, and blocks nested delegation at child depth. Automatic missions, schedules, resumption of interrupted background workflows, and the generic `delegate` agent are disabled. Tool activation stays dynamic through `subagents_enable`. The package removes completed background records after 30 days, or 90 days when their session still exists; this retention does not apply to foreground session artifacts.

Delegation defaults to `openai/gpt-6.1-sol`; `scout` and `researcher` use `openai/gpt-6-luna`, `planner` uses Sol, and `oracle` uses `openai/gpt-6-astra`. All three models are in the allowlist. The `reviewer` uses `high` thinking.

The packaged `planner`, `worker`, and `oracle` fork defaults are overridden to
fresh context. Use an explicit `context: "fork"` only when the child genuinely
needs the parent transcript; otherwise pass a compact task contract. The
`researcher` also skips inherited project instructions because its role prompt
and research task provide its operating context. Other agents retain project
instruction inheritance. When a child persists a substantial artifact, prefer
`outputMode: "file-only"` so the full result is not injected back into the
parent context.

Only `worker` retains normal implementation tools. `scout`, `researcher`,
`context-builder`, and `reviewer` have no `edit` or `write` tool. Their `bash`
access is for inspection and verification and is not a security boundary; Pi
packages and child processes still run with the current user's permissions.
For the initial rollout, use one direct foreground child at a time and do not request `workflowScript`, background runs, or worktrees. The configured global concurrency limit applies to both legacy multi-child paths and `workflowScript` children.

## Global prompts and skills

`agent-core/rules/OPERATING.md` is rendered as `APPEND_SYSTEM.md`, so it augments Pi's maintained system prompt instead of replacing it. The generated `AGENTS.md` combines shared methodology, prose guidance, and `agent-core/adapters/pi.md` without duplicating the operating invariants.

The shared [Agent skills activation](../agent-skills/README.md) installs or updates the manifest's Codex/Pi pool at `~/.agents/skills/` with pinned `skills@1.7.2` on each Home Manager switch, including when only Pi is enabled. It includes `devils-advocate`, `parallel-research-merge`, `pi-agent`, and `openai-decisions`. Before the first switch on an existing host, back up its skill files and CLI lock and resolve legacy ownership conflicts as described in [Agent core](../../../agent-core/README.md#skill-설치와-복구). Verify discovery after switching and confirm that the old `~/.pi/agent/skills/` link is absent to avoid duplicates. A successful switch needs no separate installation command. The Pi module does not own skill copies or merge logic.

## Local extensions

### Herdr parent status

When both Pi and Herdr modules are enabled, Home Manager installs `herdr-agent-state.ts` from the pinned Herdr source into `~/.pi/agent/extensions/`. This official integration reports the parent TUI session's working, idle, and blocked states. `agent_start` marks it working; `agent_settled` marks it idle only after automatic retries, compaction, and queued continuations finish. Headless child sessions do not report parent state.

Apply Home Manager and run `/reload` in Pi, or start a new Pi session inside Herdr, to load the integration. Home Manager owns this file; update it through the Herdr package pin rather than `herdr integration install pi`. The display metadata extension below does not report lifecycle state.

### Herdr subagent sidebar

`files/extensions/herdr-subagents.ts` reports the current parent session model and displays foreground `pi-subagents` children beneath their parent Pi entry in Herdr's expanded Agents sidebar. The parent row shows the agent name and model, updating when the selected model changes. Each child uses two lines: its agent name and resolved model, followed by its current tool and path, latest output, or assigned task. Both the `subagent` tool and `/run` foreground execution are supported. The layout hides pi-subagents' background summary; this extension does not enumerate background children.

The extension publishes display metadata only from the parent interactive Pi session inside Herdr. Completed, failed, interrupted, and detached foreground children disappear from the list. Session shutdown clears its tokens; active metadata expires after 45 seconds without refresh if Pi crashes. At most seven children fit in Herdr's 16-line layout. Larger groups show the first six children and an overflow count. Clicking any child line focuses the parent pane.

The progress payload and slash events follow `pi-subagents` 0.84.0. Review this integration when updating that package. It does not import the package's internal modules or modify its execution behavior. Herdr failures produce a warning and leave Pi running.

Home Manager installs the extension. After applying the configuration, run `/reload` in Pi and `herdr server reload-config` for the [Herdr sidebar layout](../herdr/README.md#subagents). For a foreground smoke test, run `/run scout List the top-level files without changing anything` inside a Herdr Pi pane and verify that the child appears, updates, and disappears when finished. Run `bun test config/modules/pi/tests/herdr-subagents.test.ts` for the isolated event and metadata tests.

### OpenAI fast mode

`files/extensions/codex-fast-mode.ts` provides:

- `/fast [on|off|status]`
- `service_tier: "priority"` for `openai` requests while enabled
- a one-line footer with working directory, Git branch, model, thinking level,
  fast-mode state, context usage, and cumulative input/output tokens

Fast mode starts enabled in each new session. Toggle changes are stored in the session so resumed branches recover their previous state. The extension is the sole owner of the custom footer and priority-mode request field. Token totals include assistant, tool, summary, compaction, and standalone usage entries; the footer caches totals until the session or its leaf changes.

Subagent child processes are detected through `PI_SUBAGENT_CHILD=1` and do
not receive the priority service tier. `PI_SUBAGENT_PARENT_SESSION` is also set
in the parent UI session for permission forwarding, so it must not be used as
the child-process signal. Parent Pi sessions continue to use fast mode normally.

The module does not install `/codex-usage` or query ChatGPT's internal Codex usage endpoint. Existing legacy credentials and historical sessions are preserved. Apply Home Manager and restart Pi to remove the previously managed usage extension.

### Prompt and cursor rendering

`files/extensions/hardware-cursor-only.ts` adds a fixed `❯` to the first visible input row and uses the active theme's `dim` color for the top and bottom borders. Built-in editor padding reserves space for the prompt, so the marker is not part of submitted text. Continuation lines, autocomplete, wrapping, and mouse positioning use Pi's editor behavior. The marker is omitted when the terminal is narrower than three columns.

The extension removes Pi 1.4's software cursor markers while preserving the terminal cursor used for IME positioning. It is paired with `showHardwareCursor: true` in `settings.json`. Review it when updating Pi if cursor rendering or IME positioning changes.

## On-demand tools

Fresh sessions use the packages' own `web_enable` and `subagents_enable` tools to activate the full web and delegation tools when needed. Package commands such as `/run` remain available. Native MCP uses Pi's `codemode` tool without declaring every server tool to the model. The module does not install a custom `enable_tools` tool or `/tool-profile` command.

When migrating an existing installation, apply Home Manager to remove its managed `~/.pi/agent/extensions/tool-profiles` symlink, then restart Pi. New sessions use the default activation state; resumed sessions may retain tools already activated in their transcript.

## LSP diagnostics

`pi-lsp` starts a configured language server only after a matching file is
written, edited, or passed to an LSP navigation tool. After a successful
built-in `edit` or `write`, diagnostics for that file are appended to the tool
result so the model can correct problems in the same turn. The package also
provides `lsp_diagnostics`, `lsp_hover`, `lsp_definition`, `lsp_references`,
and `lsp_symbols` tools.

`files/lsp.json` configures every server installed by `config/modules/lsp`, plus
`rust-analyzer` installed in the active rustup toolchain:

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

Pi selects the generated `one-half-light` theme from `config/themes/exports/pi/one-half-light.json`. The module also installs `claude-like` as an alternative. The built-in `system` theme remains available in `/settings` to derive colors from the terminal's background and ANSI palette.

The custom footer uses standard ANSI colors, so its accents follow the active terminal palette.

## Verification

After changing this module:

1. Validate edited JSON files with `jq -e . <file>`.
2. Run `bun test config/modules/pi/tests` and `bash config/modules/pi/tests/sync-settings.sh` for extension and device-local settings checks. Run `nixfmt config/modules/pi/default.nix` when the Nix module changes.
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

For the subagent rollout, also run `/subagents-doctor` and
`/subagents-models`, then verify one foreground `scout`, one `researcher`, and
a read-only `reviewer` before trying `worker` against a disposable fixture.
Confirm that no `.pi-subagents/` directory or additional Git worktree appears
in the repository.

Do not commit credentials, OAuth tokens, API keys, `auth.json`, session files,
or MCP credential caches. Pi has no built-in sandbox; packages, extensions,
skills, and tool commands run with the current user's permissions.
