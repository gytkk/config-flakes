# OMP

`modules.omp.enable` installs `pkgs.omp` and links `~/.omp/agent/config.yml` to `modules/omp/files/config.yml` in the checkout with `mkOutOfStoreSymlink`. The common Home Manager profile enables the module. `flakeDirectory` selects the checkout, normally `~/development/config-flakes`.

Only the default profile's settings file is managed. Authentication, databases, sessions, caches, and named profiles remain device-local. Pi settings and extensions are not imported, and this module does not install shared instructions. Do not put credentials or machine-specific paths in the shared YAML. Settings saved by OMP can change the checkout file; review those changes before committing.

Auto-QA tool issue reporting is disabled in the shared configuration: `dev.autoqa: false` and `dev.autoqaConsent: denied`. This disables automatic report recording and upload; it does not delete existing reports locally or from the collector. Do not enable the `PI_AUTO_QA` or `PI_AUTO_QA_PUSH` environment overrides if reports must remain disabled.

OMP is a consumer of [agent-core's shared skill target](../../agent-core/README.md#omp). Enabling OMP alone triggers [shared skill activation](../agent-skills/README.md) during Home Manager switch, even with Codex and Pi disabled. OMP discovers the installed `~/.agents/skills` pool as `agents:user`; no OMP-specific copy or extra provider configuration is needed with the managed settings. After applying the configuration, restart OMP and check `omp skill list --json`.

## MCP

MCP servers remain device-local in `~/.omp/agent/mcp.json`; this module does not manage that file. To use Context7 without importing Claude configuration, keep `claude` and `claude-plugins` in `disabledProviders` and register the server directly:

```json
{
  "mcpServers": {
    "context7": {
      "type": "http",
      "url": "https://mcp.context7.com/mcp"
    }
  }
}
```

Merge this entry into any existing server map rather than replacing it. Anonymous access works with lower rate limits; authenticated setup is described in [Context7's MCP documentation](https://context7.com/docs/resources/all-clients). Keep credentials out of the checkout. Run `/mcp reload` and `/mcp list` in OMP to load and inspect the server.

## Status line

The layout follows the Pi footer using OMP's built-in segments:

- Left: working directory, Git branch/status, context usage, input/output tokens, and subagent count.
- Right: provider quota, model/thinking level, and session mode.
- The configured separator is `pipe`; the existing `claude` composer and themes are preserved. Composer shapes can arrange the segments differently.

`usage` reports the active provider/account's available quota windows, including Codex's five-hour and seven-day windows and time until reset. Percentages are **used**, not remaining: `7d 8%` means 92% remains. Unlike Pi's CodexBar extension, this uses OMP's account reports and does not need CodexBar. Missing quota data hides the segment. Use `/usage` or `omp usage` for the full report. A narrow terminal can omit segments.

## Agent UI

- `Alt+A` or `Ctrl+S`: Agent Hub, with live status, model, cost, activity, and transcripts.
- In the Hub: `Enter` focuses an agent, `t` toggles the hierarchy, `x` stops it, and `r` revives it. Type into a focused agent's editor to send instructions; `Esc` with an empty editor returns to the main session.
- `/agents`: agent-definition/settings UI, including enable/disable and model overrides.
- `/hotkeys`: inspect active bindings if a terminal intercepts these shortcuts.

## Herdr Agents sidebar

When OMP and Herdr TUI are enabled, Home Manager installs Herdr's bundled OMP parent-state extension. The [local metadata extension](files/extensions/herdr-subagents.ts) is installed when either Herdr TUI or Herdr App is enabled. The parent reports working, idle, and blocked states, including tool approvals and `ask`; metadata never changes the parent lifecycle. Nested OMP shells and headless child sessions cannot publish over their parent.

The [Herdr TUI sidebar](../herdr/README.md#subagents) shows the parent's current model and native task/eval subagents whose events reach the extension, including background children. Child rows show the stable OMP name, resolved model, and recent intent or assistant text; tool arguments and reasoning are not displayed. Progress supplies the actual model, including fallback changes, rather than the requested model role.

Running (`●`), completed (`✓`), interrupted (`■`), and failed (`×`) runs use the same colors as the other integrations. Terminal rows remain for five seconds. Seven children fit; overflow shows six plus a count, prioritizing running children. Parent turn completion does not remove background children. Follow-up runs reset their old activity; session replacement and shutdown clear the metadata. Reports refresh every five seconds and expire after fifteen seconds if the reporter stops.

The event contract targets OMP 18.18.2's `task:subagent:lifecycle` and `task:subagent:progress` events and remains compatible with 18.18.6. Delivered nested frames are handled, but OMP 18.18.6 publishes deeper descendants on a separate root observability bus that its public extension API does not expose; complete nested-tree visibility is not guaranteed. This integration does not import OMP's private registry or scan transcripts. After an extension reload, already-running children appear on their next progress event; no event-history snapshot is available.

Apply Home Manager, run `herdr server reload-config`, and restart OMP inside Herdr. `herdr integration status` should list OMP as current. No Herdr server restart is required for these extension/config changes.

```bash
bun test modules/omp/tests
```

### Herdr Desktop

The same event/state reducer feeds the [personal Desktop Agents mod](../herdr/README.md#desktop-subagents). Outside a valid TUI pane, the extension identifies its parent using `HC_TERMINAL_ID` and publishes a versioned JSON snapshot to that core terminal's `omp.subagents` label. `HC_TERMINAL_SOCKET` takes precedence over `HC_SOCKET`, preventing a terminal ID from being resolved on the wrong core. Inside a TUI pane, only the TUI transport runs: several TUI panes can share one core terminal.

Desktop snapshots retain all visible children rather than the TUI's seven slots. Each session replacement gets a new identity; queued writes are serialized before replacement and shutdown deletion. Refresh and expiry are five and fifteen seconds, respectively, and settled children disappear after five seconds. Core labels are persistent state: activity excerpts can remain on disk after a crash until replaced or cleared, even though the UI hides stale snapshots. Remote reporters and the desktop need synchronized clocks for timestamp-based expiry.

Apply Home Manager on the reporter's host and the desktop host, select the personal `tui` layout, and restart OMP to load the extension. Desktop does not need `herdr server reload-config`. Other runtimes' TUI reporters are not converted by this integration.

## Apply

Run from the repository root. On the first standalone Home Manager switch, preserve an existing regular configuration file with a unique backup suffix:

```bash
home-manager switch -b before-omp-module --flake .#pylv-denim
```

Use the appropriate environment on other hosts. If that backup already exists, choose another suffix. Review each host's existing settings before switching; merge any desired differences into the shared YAML. Do not use `home.file.force` to discard local settings. NixOS users must preserve any existing conflicting file before their normal system switch.

Restart OMP after activation. `readlink -f ~/.omp/agent/config.yml` should resolve to the checkout file. OMP authentication remains independent of Pi; use `/login` if needed.

## Verification

From the repository root:

```bash
nixfmt systems/base/default.nix modules/omp/default.nix
nix eval .#homeConfigurations.pylv-denim.config.modules.omp.enable
uv run --no-project scripts/check-paths.py modules/omp/README.md
omp config list --scope global --json
omp gallery --surface segment --segment usage --plain --width 160
```

The gallery uses fixture quota values, not the logged-in account's remaining quota. Before activation, configuration parsing and an interactive launch can be checked with a temporary `PI_CODING_AGENT_DIR` containing a copy of the shared YAML, without modifying the live profile.
