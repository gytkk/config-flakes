# Herdr

[default.nix](default.nix) installs Herdr and generates the read-only `~/.config/herdr/config.toml` from [files/config.toml](files/config.toml) and the [shared theme export](../../themes/exports/herdr). The prefix is `Ctrl+Space`.

The custom palette follows `modules.commonTheme`, including its secondary text colors. Automatic theme switching is disabled. Home Manager owns the generated config, so Herdr's UI cannot persist theme, sound, toast, or status settings. Edit the source configuration and apply Home Manager to change these settings.

The current space and agent use a row background derived from the sidebar and normal text colors, with at least 1.8:1 contrast against the sidebar. One Half Light uses a darker gray row background; the keyboard navigation cursor retains its separate selection color. The generator keeps normal and secondary text readable on focused rows and adjusts the selected-tab accent to provide at least 4.5:1 contrast with its label color. One Half Light uses light labels on a darker blue selected tab. The source colors and contrast rules live in the [canonical theme pipeline](../../themes/README.md#nix-integration).

Running agents and Claude/Codex subagents use vivid yellow (`#ffe600`); idle/done agents use bright green (`#00e040`). Parent status icons and running subagent icons have a dark background (`#383a42`) confined to the icon cell, so these colors remain visible on light and dark rows. The colors are defined in the [Herdr template](../../themes/templates/herdr/official-template.json), with literal hex colors in subagent rules. Regression tests check at least 4.5:1 contrast on the icon background using both sRGB and Ghostty Metal calculations. The [token background patch](../../packages/apps/herdr/sidebar-token-background.patch) adds the required `bg` styling to Herdr 0.9.5. Apply Home Manager and restart the Herdr server when its running tasks can be interrupted; configuration reload alone cannot load this binary change.

## Terminal shortcuts

| Shortcut | Action |
| --- | --- |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Focus the next / previous tab in the current workspace. |
| `Cmd+[` / `Cmd+]` | Focus the previous / next workspace in Ghostty on macOS. |
| `Cmd+Shift+[` / `Cmd+Shift+]` | Focus the previous / next agent in sidebar order in Ghostty on macOS, wrapping at either end. |
| `Cmd+t` | Create a Herdr tab without a naming prompt in Ghostty on macOS. |
| `Cmd+w` | Close the current Herdr tab in Ghostty on macOS. |
| `Alt+t` / `Alt+w` | Create / close a Herdr tab using Herdr's own bindings, including in Windows Terminal on WSL. |
| `Cmd+1..9` / `Alt+1..9` | Focus a workspace by index in Ghostty on macOS / Windows Terminal on WSL. |

The prefix shortcuts remain available: `c` creates a tab, `Shift+x` closes a tab, `p` / `n` focuses the previous / next tab, and `Shift+,` / `Shift+.` focuses the previous / next agent. `Ctrl+Space`, then `Shift+s`, opens the workspace picker.

`Alt+t` and `Alt+w` are handled by Herdr without Windows Terminal overrides. Outside Herdr, these keys retain their shell or application behavior.

In Herdr 0.9.1, closing a tab terminates its panes. Closing the last tab also closes its workspace, normally without confirmation. With the default `confirm_close = true`, closing the last tab prompts when it would close a linked worktree group. Direct shortcuts apply in terminal mode; popups and overlays handle their own input.

[Ghostty's config](../ghostty/files/config) overrides its native tab, split navigation, and close shortcuts with explicit Kitty keyboard sequences. Physical key bindings preserve the Command shortcuts while an IME is active. These overrides apply throughout Ghostty, including outside Herdr; the keys no longer perform their original Ghostty actions there.

[Windows Terminal's config](../windows-terminal/files/herdr-keybindings.json) forwards `Ctrl+Tab`, `Ctrl+Shift+Tab`, and `Alt+1..9`. Its next/previous tab actions retain their existing `User.herdrNextWorkspace` and `User.herdrPreviousWorkspace` IDs so activation replaces the previously managed actions.

## Agent detection

When both Pi and Herdr modules are enabled, the [Pi parent status integration](../pi/README.md#herdr-parent-status) reports working, idle, and blocked states through Herdr's bundled extension. Home Manager installs the version matched to the pinned Herdr package. Pi's model and subagent display metadata does not set the parent lifecycle state.

The packaged Herdr excludes direct `claude plugin ...`, `claude mcp ...`, `claude --version` / `-v`, and `claude --help` / `-h` invocations from process-based agent detection. Home Manager runs plugin and MCP management commands during `setupClaudeCode`; they do not represent an interactive Claude session. Normal Claude sessions, including resume and continue, remain detectable. Unrecognized argument layouts retain upstream detection behavior.

This behavior comes from [the local detection patch](../../packages/apps/herdr/claude-admin-detection.patch). After applying Home Manager, restart the Herdr server when its running tasks can be interrupted to load the patched binary. `herdr server reload-config` only reloads configuration.

## Subagents

Agent entries in [files/config.toml](files/config.toml) show the agent name and reported model on their second row. Codex reads the parent's latest `turn_context.model`, Pi reports its current session model, and [Claude's status line](../claude/README.md#settingsjson) reports its selected model. Claude's model expires after 45 seconds without a status line update and may disappear while idle. Missing model metadata is hidden; agents without a model reporter show only their name. Parent rows do not display summaries or child counts.

The Claude, Pi, and Codex layouts also display child names, models, and activity. The [Pi extension](../pi/README.md#herdr-subagent-sidebar) reports foreground children. The [Codex hook and watcher](../codex/README.md#herdr-subagent-sidebar) read direct child sessions from local rollout files. The [Claude hook and watcher](../claude/README.md#herdr-subagent-sidebar) read native child transcripts and parent results. Both watchers include subagents that continue in the background. Empty child rows are hidden.

Codex shows a colored status icon, name, and model on each child's first line, with an excerpt of its latest assistant message below it in the generated theme's secondary text color. The [local separator patch](../../packages/apps/herdr/sidebar-status-separator.patch) places one space after custom lifecycle and overflow icons, without a dot separator. Load this patch by applying Home Manager and restarting the Herdr server when its running tasks can be interrupted; config reload alone does not load binary changes. The excerpt is limited to 80 characters with an ellipsis when truncated; tool events preserve it, and task completion uses the final response when available. Fixed labels are English; messages retain their original language. Running, completed, interrupted, and failed are separate states. Terminal states remain visible for five seconds. Read failures show an unknown state and the last message. If the watcher dies or cannot reach Herdr, its last metadata expires within 15 seconds; expiration removes rows rather than generating an unknown icon.

The expanded desktop sidebar supports seven children with two lines each. If more children are visible, it shows six plus an overflow count. Codex prioritizes running and unknown children before recent terminal states. The rows belong to the parent pane and do not create additional terminal panes. Collapsed and mobile layouts remain compact.

Claude uses the same two-line layout and status colors. Its `SubagentStop` hook is tentative: `○` remains until a parent result or task notification confirms completion, failure, or interruption. For Agent Teams, removal from a valid team configuration led by the current session also confirms interruption. Task descriptions replace agent types when an Agent result arrives, and messages update from child transcripts. Confirmed terminal states remain visible for five seconds, and stale metadata expires within 15 seconds. Sidebar cleanup preserves worktrees.

Apply Home Manager, run `herdr server reload-config`, and start a new Claude, Pi, or Codex session inside Herdr to load the integration.

## Shared JSONL reader

The Codex and Claude watchers use [files/jsonl-tail.ts](files/jsonl-tail.ts) for incremental JSONL reads. The reader buffers incomplete UTF-8 records, resets offsets after file replacement or truncation, and diagnoses malformed or oversized records without logging their contents. Each runtime module packages its watcher and this shared file with their repository-relative paths intact; the reader does not require the Herdr module to be enabled.

Run the shared reader and runtime regression tests with:

```bash
bun test modules/herdr/tests modules/codex/tests modules/claude/tests
```

## Apply changes

Herdr, Annotate, and Auto Title updates are proposed together through the [app package review channel](../../packages/README.md#updates-and-ci). Herdr's local patches are checked before its source pin changes; Annotate uses unpatched upstream sources. Candidate PRs run Linux package CI before manual merging. An open candidate is preserved for fixes; other app updates proceed independently. After merging a candidate, pull its changes before applying Home Manager.

Apply Home Manager for the current environment to install the terminal config, for example `home-manager switch --flake .#devsisters-macbook`. Reload Ghostty with `Cmd+Shift+,`, then run `herdr server reload-config` to reload the running Herdr server's bindings and theme. Changes to Herdr's source configuration or shared theme exports take effect after applying Home Manager and reloading Herdr.
