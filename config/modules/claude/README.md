# Claude Code Module

This module installs and configures Claude Code, Anthropic's AI coding assistant.

## What it does

- Installs `claude-code` from the repository's [app package catalog](../../packages/README.md), exposed through the configuration overlay
- Configures Claude Code settings (`~/.claude/settings.json`)
- Installs global development guidelines (`~/.claude/CLAUDE.md`)
- Installs plugin marketplaces, plugins, and MCP servers via activation scripts
- Installs plannotator CLI for visual plan review

[activation.nix](activation.nix) generates the marketplace, plugin, and MCP setup shell. `default.nix` owns their selections and Home Manager activation ordering; `plugin-updates.py` owns update caching.

Each Home Manager activation runs the official plannotator installer with `--minimal` to install or update the CLI in `~/.local/bin`. The installer skips agent integrations, which this module manages through the Claude plugin. Installation failures are logged in `~/.claude/nix-setup.log` and do not stop activation.

## Configuration Files

### settings.json

`~/.claude/settings.json` uses an out-of-store symlink to [files/settings.json](files/settings.json) in every profile. Changes saved to user settings from Claude Code update that tracked file directly. Project and project-local settings remain scoped to their projects. Home Manager temporarily uses a writable copy during marketplace registration and plugin installation and updates, then restores the symlink so those activation operations do not rewrite the shared settings.

- **Model**: Inherits the Claude Code default (no `model` pin)
- **Theme**: `light-ansi` uses the terminal's ANSI palette on a light background
- **Agent Teams**: Enabled (`CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`)
- **MCP**: Enables all project MCP servers and Context7; Notion uses the `ntn` CLI
- **Permissions**: Pre-approved tools (Bash, Read, Edit, Write, WebFetch, WebSearch, Context7)
- **Permission Mode**: `acceptEdits` for the working directory and Claude default repo-local worktrees
- **Memory** (experimental): `autoMemoryEnabled` + `autoDreamEnabled` — native background insight extraction and 24h consolidation
- **Status line**: Working directory, Git branch, context/tokens/line changes, plus the Claude.ai weekly limit, selected model, and effort right-aligned
- **Language**: English
- **Telemetry**: OTLP metrics and logs go to the Devsisters Databricks workspace; endpoints and the per-signal `X-Databricks-UC-Table-Name` headers are static `env` values

The Databricks bearer token is never stored in `settings.json`. `otelHeadersHelper` points at `~/.local/bin/claude-otel-headers`, which Claude Code runs at startup and every 29 minutes. The script returns `{"Authorization": "Bearer ..."}` read from `modules.claude.otelTokenFile`, and the Devsisters profile sets that option to the agenix-decrypted `databricks-token` secret. Claude Code merges the helper output over the static per-signal headers. When the option is null the helper returns `{}`; when the token file is unreadable it exits non-zero, which Claude Code reports in `/status`.

Inside Herdr, the status line also reports the selected model for the [Agents sidebar](../herdr/README.md#subagents). The model updates whenever Claude invokes the status line and expires after 45 seconds without an update, so it may disappear while Claude is idle. Missing model data clears the value. Reporting failures leave the terminal status line intact.

### CLAUDE.md

Global development guidelines are rendered from shared rules under `agent-core/rules/` and `agent-core/adapters/claude.md`, then deployed to `~/.claude/CLAUDE.md`.

The [Claude-specific instructions](../../../agent-core/adapters/claude.md#response-language) require messages with two or more paragraphs before translation to include the English original followed by a complete Korean translation. Apply Home Manager and start a new Claude Code session to load instruction changes.

## Herdr subagent sidebar

Inside Herdr, Claude's Agents entry displays native subagents with a status icon, name, and actual model, followed by an excerpt of the latest assistant message in the generated Herdr theme's secondary text color. Before a message is available, the activity can show the tool name. Thinking blocks, tool arguments, and tool results are not displayed. Children share the parent's pane; this integration does not create terminal panes.

[settings.json](files/settings.json) connects `SessionStart`, `SubagentStart`, `SubagentStop`, `PostToolUse` for `Agent`, and `SessionEnd` to `~/.local/bin/claude-herdr-subagents`. The [helper](files/herdr-subagents/index.ts) runs one watcher per Herdr pane and reads the current session's parent and child JSONL records. Background children continue to be watched after the parent finishes responding. Agent results supply task descriptions; until a result is available, the name is the agent type and a short ID. Resuming the same child updates its existing row.

JSONL reads use the [shared reader](../herdr/README.md#shared-jsonl-reader) also used by the Codex watcher. Nix packages the watcher and shared file together; transcript discovery and state projection remain in the Claude module.

Running uses a green `●`, confirmed completion a blue `✓`, interruption a yellow `■`, and failure a red `×`. `SubagentStop` alone shows `○` while the watcher waits for a parent result or Claude's task notification, because another stop hook can request more work. These states describe a run, not permanent termination of the agent. Completed, interrupted, and failed rows remain visible for ten seconds. Rows waiting for confirmation are also hidden ten seconds after `SubagentStop`; this does not mark them completed, and new activity makes them visible again. Hidden rows do not count toward the display limit. Missing or unreadable transcripts show `○` and retain the last message while the row is visible. If the watcher stops reporting, metadata expires within 15 seconds. The parent model remains independently reported by the status line.

For Agent Teams, the watcher reads each stopped child's native `.meta.json` and the team's `config.json` under `CLAUDE_CONFIG_DIR` (default: `~/.claude`). Removal from a team led by the current session confirms interruption, so the row disappears ten seconds after confirmation. Registered teammates follow the same ten-second hiding rule after `SubagentStop`. Missing, unreadable, or invalid team files do not confirm termination. Worktrees remain unchanged.

The parser targets the pinned Claude Code 2.1.284 transcript format. Stop-only events from internal agents do not create rows. Independently launched Claude processes are not linked as children. Display limits are documented in [Herdr's subagent settings](../herdr/README.md#subagents).

Apply Home Manager for the current environment, run `herdr server reload-config`, and start a new Claude session inside Herdr. Verification uses isolated fixture transcripts and fake Herdr commands:

```bash
bun test config/modules/herdr/tests config/modules/claude/tests
```

## Skills and plugins

Activation checks each configured Git marketplace's remote revision against its local checkout and updates only changed indexes. For archive snapshots without Git metadata, it compares the revision and local contents with the last successful update. Local directory marketplaces are read directly. The [update helper](plugin-updates.py) skips a plugin's install/update command when its source revision or local contents, marketplace entry, Claude executable, and installed files match the last successful operation. File fingerprints include permissions while accounting for activation's shell executable-bit repair. External plugin repositories are checked even when the marketplace index is unchanged; pinned sources retain their declared revision. Revision checks still require network access. Unsupported sources and failed checks fall back to Claude's update command, and failed operations are not cached. Claude remains responsible for deciding whether a declared plugin version requires replacement.

The helper stores disposable success records in `$XDG_CACHE_HOME/claude-nix/plugin-updates.json`, defaulting to `~/.cache/claude-nix/plugin-updates.json`. The first switch populates this cache. Missing installations, changed installed files, and changed sources trigger another CLI operation. Setup output remains in `~/.claude/nix-setup.log`. Run the isolated cache tests with `uv run --no-project -m unittest discover -s config/modules/claude/tests -p 'test_*.py'`; these tests do not contact remote services or change the user's plugins.

The shared `openai-decisions` skill guides typed judgments with the `gpt-6-luna` Decisions API. Invoke it with `/openai-decisions` and run API clients through `with-openai` to use the existing agenix OpenAI key. See the [OpenAI API README](../openai-api/README.md) for authentication and activation.

The shared [Agent skills activation](../agent-skills/README.md) installs or updates Claude's selected skills with pinned `skills@1.7.2`, using `--copy --global` into `~/.claude/skills/` on each Home Manager switch. Before the first switch on an existing host, back up its skill files and CLI lock and resolve legacy ownership conflicts as described in [Agent core](../../../agent-core/README.md#skill-설치와-복구). Verify discovery after switching; a successful switch needs no separate installation command. The local `devils-advocate` marketplace plugin remains separately owned because it provides a Claude command, agent, and plugin metadata.

### Marketplaces

All remote marketplaces listed below set `autoUpdate: true`, including third-party marketplaces. `FORCE_AUTOUPDATE_PLUGINS=1` allows plugin updates while the Nix package disables Claude Code's own updater. Activation registers the local `gytkk` marketplace from the configured checkout path in each host's `~/.claude/plugins/known_marketplaces.json`; the shared settings omit that host-specific path. Manually adding a local marketplace through Claude Code still writes its path to the shared settings. See [Claude Code's auto-update rules](https://code.claude.com/docs/en/plugins/loading#which-marketplaces-and-plugins-auto-update).

The `devsisters` base profile enables `modules.claude.mumu.enable`, which registers `devsisters/mumu` and installs and enables `mumu@mumu`. Other profiles leave this option disabled. With mumu enabled, the installed `claude` launcher passes a generated file containing only the mumu entries through [`--settings`](https://code.claude.com/docs/en/settings#change-a-setting-for-one-session). The shared user settings remain writable through the checkout symlink. The command-line mumu settings override user and project settings, so disabling mumu in Claude Code does not override the launcher; change `modules.claude.mumu.enable` and apply Home Manager instead. Launching the underlying Nix package directly bypasses this profile overlay. Disabling the option stops managing mumu installation but does not uninstall an existing plugin or remove its cached marketplace.

- `anthropics/skills`, `anthropics/claude-code`, `anthropics/claude-plugins-official`
- `gytkk` (local marketplace — `config/modules/claude/marketplace`)
- `backnotprop/plannotator`
- `openai/codex-plugin-cc`
- `DietrichGebert/ponytail`, `pbakaus/impeccable`
- `devsisters/mumu` (Devsisters profile only)

### Installed Plugins

- `document-skills`, `commit-commands`, `security-guidance`
- `devils-advocate` (runtime-specific multi-pass review command and agent)
- `ralph-loop`, `superpowers`
- `ponytail@ponytail` (minimal implementation guidance, review commands, and session hooks)
- `impeccable@impeccable` (frontend design skill and automatic design checks)
- `slack` (Slack's hosted MCP server plus Slack developer skills)
- LSP plugins: `gopls-lsp`, `rust-analyzer-lsp`, `typescript-lsp`, `pyright-lsp`, `terraform-ls`, `nixd-lsp`
- `plannotator` (visual plan annotation and review)
- `mumu@mumu` (Devsisters profile only)
- `codex` (official OpenAI Codex plugin — provides `/codex:review`, `/codex:rescue`, etc.)
- `cc-plugin-you-should-know@builtin` (bundled side agent that flags easily missed information above the prompt). Built-in plugins ship inside Claude Code, so they are enabled only through `enabledPlugins` in `files/settings.json`, not through the `plugins` install list in `default.nix`.

The official `pyright-lsp` plugin uses `pyright-langserver --stdio` from [the LSP module](../lsp/default.nix) for Python code intelligence. `ty-lsp@gytkk` is explicitly disabled because Claude uses one server per file extension; the `ty` package remains installed for other clients. Apply Home Manager and start a new Claude session to load the package and plugin settings.

## MCP Servers

- **context7**: Library documentation lookup
- **slack**: Provided by the `slack` plugin, not by `claude mcp add`. The plugin
  ships a pre-registered OAuth client id for `https://mcp.slack.com/mcp`;
  registering that URL directly would fail because Claude Code only performs
  Dynamic Client Registration, which Slack does not support. Authenticate with
  `/mcp` inside Claude Code. A Slack workspace admin must approve MCP
  integration for the workspace before authentication succeeds.

## Notion

Use the `ntn` CLI for Notion pages, data sources, and API actions. The Notion MCP
server is intentionally removed during activation.

Cloudflare operations use the `cf` CLI installed by the common Home Manager profile. Activation removes the legacy user-scope `cloudflare` MCP registration. See [Cloudflare CLI setup](../../README.md#cloudflare-cli) for authentication and command discovery.

## Browser use

The [shared instructions](../../../agent-core/rules/AGENTS.md#tool-specific-workflows) direct Claude Code to open pages, local previews, and authentication links with the system default browser. Browser automation and testing use `agent-browser`, installed by the common Home Manager profile, in headless mode by default. These are agent instructions, not execution restrictions. Activation removes the user-scope `playwright` MCP registration. Apply Home Manager and start a new Claude Code session to load the instructions.

## Usage

```bash
# Run Claude Code
claude
```
