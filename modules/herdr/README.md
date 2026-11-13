# Herdr

[default.nix](default.nix) installs Herdr and generates the read-only `~/.config/herdr/config.toml` from [files/config.toml](files/config.toml) and the [shared theme export](../../themes/exports/herdr). The prefix is `Ctrl+Space`.

The custom palette follows `modules.commonTheme`, including its secondary text colors. Automatic theme switching is disabled. Home Manager owns the generated config, so Herdr's UI cannot persist theme, sound, toast, or status settings. Edit the source configuration and apply Home Manager to change these settings.

The current space and agent use a row background derived from the sidebar and normal text colors, with at least 1.8:1 contrast against the sidebar. One Half Light uses a darker gray row background; the keyboard navigation cursor retains its separate selection color. The generator keeps normal and secondary text readable on focused rows and adjusts the selected-tab accent to provide at least 4.5:1 contrast with its label color. One Half Light uses light labels on a darker blue selected tab. The source colors and contrast rules live in the [canonical theme pipeline](../../themes/README.md#nix-integration).

Running status green has a 1.8:1 sRGB contrast target against sidebar and active-row backgrounds, leaving margin above [Ghostty's configured minimum](../ghostty/files/config) of 1.6. Ghostty 1.3.1's [Metal renderer](https://github.com/ghostty-org/ghostty/blob/v1.3.1/src/renderer/shaders/shaders.metal) converts sRGB colors to Display P3 before checking contrast, so a passing sRGB ratio alone does not guarantee that green survives its black or white fallback. One Half Light uses `#4e974e` for the theme's running green and the Claude/Codex subagent rules. Herdr requires literal hex colors in those rules. Regression tests check both sRGB and Metal contrast calculations on sidebar and active-row backgrounds.

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

`Ctrl+Space`, then `Shift+n`, opens an fzf directory picker populated by zoxide. The new workspace starts in the selected directory and uses its final directory name as the label, such as `keycult-trackball` for `~/keycult-trackball`. Existing workspace labels remain unchanged.

`Alt+t` and `Alt+w` are handled by Herdr without Windows Terminal overrides. Outside Herdr, these keys retain their shell or application behavior.

In Herdr 0.9.1, closing a tab terminates its panes. Closing the last tab also closes its workspace, normally without confirmation. With the default `confirm_close = true`, closing the last tab prompts when it would close a linked worktree group. Direct shortcuts apply in terminal mode; popups and overlays handle their own input.

[Ghostty's config](../ghostty/files/config) overrides its native tab, split navigation, and close shortcuts with explicit Kitty keyboard sequences. Physical key bindings preserve the Command shortcuts while an IME is active. These overrides apply throughout Ghostty, including outside Herdr; the keys no longer perform their original Ghostty actions there.

[Windows Terminal's config](../windows-terminal/files/herdr-keybindings.json) forwards `Ctrl+Tab`, `Ctrl+Shift+Tab`, and `Alt+1..9`. Its next/previous tab actions retain their existing `User.herdrNextWorkspace` and `User.herdrPreviousWorkspace` IDs so activation replaces the previously managed actions.

## Herdr App shortcuts

[The desktop module](../herdr-app/default.nix) manages shortcuts for the separately installed Herdr App; it does not install or update the app or herdr-core. It is enabled by default on macOS and WSL. The TUI configuration and server remain independent.

On WSL, every Home Manager switch runs the installer against the Windows app's `/mnt/c/Users/<username>/.config/herdr-app`, using the configured environment username, as the Windows Terminal and Zed modules do. It does not target the WSL user's app configuration or require Windows executable interop. Launch the Windows app once first; if its `keys.json` is absent, installation is skipped with a message. Unchanged files are left alone, and changed files receive `.home-manager.bak` backups. Select the `tui` layout once from Windows with the command below. Outside WSL, the target remains the Home Manager XDG config directory.

For Windows desktop work in WSL, connect the distro as a Herdr machine instead of starting `wsl.exe` inside each Windows terminal. Herdr core 0.1.20 or newer supports this without SSH. From Windows PowerShell 7, register the machine once and create a space on it:

```powershell
hc machine add --via 'wsl.exe -d Ubuntu -e sh -lc' --name wsl --install
herdr-app do space.new --machine wsl
```

The connection targets the installed `Ubuntu` distribution explicitly. The login shell initializes the Nix PATH when connecting; it is not started afresh for each tab. Machine registration is mutable Windows core state, not recreated by Home Manager. Existing Windows-local spaces are not migrated or closed: select a space under the `wsl` machine to work in Linux. Leave the space name unset so Herdr follows its repository or folder instead of showing a fixed `WSL` label; clear an existing space's name to restore this automatic naming.

The personal `tui` sidebar shows This computer first, then the machine named `wsl`, then the remaining machines alphabetically. In this layout, `Alt+1..9` and `Cmd+1..9` select spaces in that same grouped sidebar order, including worktree placement, rather than the core's machine order. Compact sidebar labels use those same numbers. Empty machine headers consume no number; folding a machine keeps its numbers reserved, and an unused number does nothing. Other layouts keep Herdr's native numbered selection. Space ordering inside each machine is unchanged; core machine and space identifiers are not rewritten.

In that space, `Alt+t` and `Ctrl+Space`, then `c`, use the WSL core's default shell and inherit the focused terminal's directory. `Ctrl+Space`, then `Shift+n`, queries zoxide on the originating machine and opens the selected directory on that same machine. Paths stay literal Linux paths; no PowerShell or per-tab `wsl.exe` wrapper is used. Menu-created tabs and splits also stay on the space's machine. Other remote machines and Windows-local spaces retain their native behavior.

If an existing WSL core was started without the Nix PATH or the intended `SHELL`, refresh it from a normal configured WSL terminal outside Herdr with `hc server restart --fresh`. Existing terminals keep running. Preserve `SHELL` when doing this: an unset value makes new terminals use `/bin/sh`; for this Ubuntu setup, `SHELL=/usr/bin/zsh hc server restart --fresh` restores zsh. Check the connection with `hc machine ls` and run `hc exec --on wsl zoxide query -l` from Windows to verify the remote program environment.

Launch the app once before applying Home Manager. The [installer](../herdr-app/install.py) merges [managed keys](../herdr-app/files/keys.json) into the existing desktop key file, installs the personal [prefix mod](../herdr-app/files/mods/tui-prefix/main.js) and [OMP-aware Agents mod](../herdr-app/files/mods/omp-agents/main.js), and creates the `tui` layout from the installed `main` layout if missing. On later applications it updates that layout's status block, replaces its standard `agents` blocks with `omp-agents`, and sets the pane gap, terminal padding on all sides, and the surrounding `work-body` padding to 4 px, with normal/focused pane border widths at 0.5 px. Outer spacing means the padding around the pane area, not border thickness. Other edits, custom nonstandard agent panels, and the shipped layout are preserved. The gap applies to both side-by-side and stacked splits; pane headers, border colors, corner radii, and the gap to optional side panels are unchanged. Changed files retain their previous contents in `.home-manager.bak` files; unrelated keys, settings, and session state are preserved. Home Manager dry-run does not run the installer.

Select the personal layout once with `herdr-app do layout.switch --name tui`. The choice persists. In that layout, `Ctrl+Space` focuses the existing bottom status line and shows **PREFIX** with a few useful shortcuts, without opening a popup or resizing the terminal. There is no timeout. A command, `Esc`, a second `Ctrl+Space`, an unknown key, or moving focus away leaves prefix mode. Unknown prefix keys are consumed, not typed into the terminal. Pressing `Ctrl+Space` again only cancels prefix mode and restores terminal focus; it does not send input to the terminal. Ordinary typing is not intercepted outside prefix mode.

The bottom status line uses JetBrains Mono in both idle and prefix modes, with the existing heavier badge and key labels. Install that font on the desktop host for the intended typography; this setting does not change terminal text, other app panels, or picker dialogs.

While prefix mode is active, the status line uses golden amber: the current theme's ANSI yellow/ochre mixed 55% toward `#ffb300`. This amber anchor keeps the highlight visibly golden rather than brown even in themes with dark ochre. The bar blends 15% of that accent into the normal chrome color; the compact `PREFIX` badge selects black or white text for contrast. The default Paper theme gives a `#cd8c00` badge with black text and a `#ece2ca` bar. Colors return to normal on exit; the theme's ochre and chrome still influence the active colors.

| After `Ctrl+Space` | Action |
| --- | --- |
| `c`, `Shift+x`, `Shift+t` | New, close, rename tab |
| `p`, `n`, `1..9` | Previous, next, numbered tab |
| `Shift+s`, `Shift+n` | Space picker; create a space from zoxide history |
| `Shift+w`, `Shift+d`, `Shift+g` | Rename space, close space, create worktree |
| `Shift+a`, `<`, `>` | Agent picker, previous agent, next agent |
| `Shift+1..9` | Agent in space/tab/pane order, as numbered in the agent picker |
| `h/j/k/l`, `Shift+h/j/k/l` | Focus or swap panes |
| `Tab`, `Shift+Tab` | Next or previous pane |
| `v`, `-`, `x`, `z` | Split right, split down, close pane, zoom |
| `b`, `s`, `?` | Toggle sidebar, settings, prefix help |

`Ctrl+Space`, then `q`, is unbound: it only leaves prefix mode and restores terminal focus, without closing the window or sending input.

Only pickers and explicitly requested help open dialogs. The zoxide picker uses a native filter rather than fzf and queries the originating space's machine. Agent numbers follow the App API's space/tab/pane order, not its priority-sorted sidebar. Direct `Cmd+[/]`, `Cmd+Shift+[/]`, `Cmd+1..9`, `Cmd+t/w`, `Alt+t/w`, and `Ctrl+Tab` / `Ctrl+Shift+Tab` match the TUI actions above across layouts. Prefix mode is scoped to `tui`; other layouts keep normal `Ctrl+Space` input.

In the desktop App, `Alt+Up` / `Alt+Down` and `Alt+[` / `Alt+]` also focus the previous / next space, including on Windows. The bracket shortcuts replace Windows' default previous / next tab bindings; `Ctrl+Shift+Tab` / `Ctrl+Tab` remain available for tab navigation.

For prefix-free pane splitting on macOS, the locally observed App `keys.json` binds `Cmd+d` to `pane.split:right` and `Cmd+Shift+d` to `pane.split:down`. These are App-side bindings, not TUI shortcuts or overrides in the repository's managed keys. The installer preserves existing bindings for these keys; it does not enforce their presence on another host or replace that host's shipped defaults. Check the App's own `keys.json` if they differ. The managed prefix alternatives remain `Ctrl+Space`, then `v` or `-`.

This is not a TUI plugin migration. TUI copy/resize modes, pane renaming, and editing scrollback are not mapped to unrelated App actions. App files hot-reload; no `herdr server reload-config` is needed. The integration targets App 0.1.15's mod API.

The desktop installer manages only `minimum_contrast` in `settings.json`, setting it to `3.0` while preserving other personal settings. Herdr replaces terminal text below this contrast threshold with near-white or near-black, making Claude Code's submitted prompts readable on their dark background without replacing as many ANSI colors as a `4.5` threshold. This affects all desktop terminals, not only Claude; it is a correction threshold, not a guaranteed final contrast ratio. The app hot-reloads it, including existing text. Changed settings are backed up to `settings.json.home-manager.bak`.

Checks from the repository root:

```bash
node --experimental-vm-modules --test modules/herdr-app/tests/*.test.mjs
python3 -B -m unittest discover -s modules/herdr-app/tests -p 'test_*.py'
herdr-app check modules/herdr-app/files/mods/tui-prefix
herdr-app check modules/herdr-app/files/mods/omp-agents
herdr-app check modules/herdr-app/files/mods/tui-spaces
```

### OMP file links in Neovim

On macOS, enabling OMP, Herdr App, and Neovim installs `~/Applications/OMP Neovim.app` as the handler for `omp-nvim:` links. This does not change Finder's file associations, the default browser, or Herdr's shipped code. The [OMP launcher](../omp/default.nix) adds the custom file-link scheme only when the current local Core terminal belongs to an App pane; SSH, Herdr TUI, and ordinary terminal launches keep their existing links.

Cmd-click an OMP-generated file hyperlink to open text or source code in a new right-hand Neovim pane. Later clicks from the same originating pane reuse that editor and honor line/column positions. Switching files preserves unsaved buffers. Closing the editor lets the next click create a fresh pane; a closed origin is rejected rather than targeting whichever pane is focused. Images, PDFs, other non-text files, and directories retain macOS's default opener, and web links remain unchanged. Content MIME detection, not the filename extension alone, distinguishes text from binary files.

The [router](../herdr-app/file-opener.py) validates the local computer and origin pane, passes filenames as data, and uses Neovim RPC rather than typing shell commands. Its private `/tmp/omp-nvim-<uid>` directory holds editor sockets and per-origin pane records. This is a local desktop integration: it does not retrieve remote files or change plain paths that Herdr detects independently of OMP hyperlinks.

Apply Home Manager and start a new OMP process. Existing OMP sessions read their link scheme only at startup; reloading extensions is not enough. The [installer](../herdr-app/install-file-opener.py) registers only this custom scheme. It compiles and signs the new handler before changing the installed application, then keeps the preceding managed application as `.home-manager.bak` after successful registration, replacing an older managed backup. Failed publication or registration restores the preceding application and backup; an unchanged reinstall leaves both untouched. Unrelated applications and backups are not replaced. If rollback itself fails, the error identifies a retained `.omp-file-opener-*` recovery directory beside the application; preserve its contents for manual recovery. Before a Home Manager switch, the installed handler can be exercised by launching from this checkout:

```bash
python3 modules/herdr-app/file-opener.py launch "$(command -v omp)"
```

The file-opening regression check uses real isolated Neovim processes and temporary files, including literal filenames and modified-buffer preservation. The installer regression check uses temporary application bundles and stubbed compile, sign, and registration commands; it never touches the real application registry:

```bash
python3 -B -m unittest discover -s modules/herdr-app/tests -p test_file_opener.py -v
python3 -B -m unittest discover -s modules/herdr-app/tests -p test_install_file_opener.py -v
```

### Desktop space completion indicators

The personal `tui` layout replaces standard `spaces` blocks with [tui-spaces](../herdr-app/files/mods/tui-spaces/main.js). Each space shows a small blue circle on its right when at least one of its agents has an unread completed turn (`done`). The indicator uses the theme's ANSI blue in both expanded and compact sidebars. It aggregates individual agents rather than the space's priority status, so a blocked or working agent cannot hide another agent's completion. Machine-qualified space IDs keep local and remote spaces separate.

Herdr owns read tracking: viewing an agent clears its `done` state; the circle remains until no unread completions remain in that space. This is not a history of all completed turns, nor an indicator for OMP metadata-only subagent rows. Installation preserves custom space mods and the shipped layout. The personal mod retains an Apache-2.0-licensed copy of App 0.1.15's Spaces implementation; review upstream changes when updating App versions.

### Desktop subagents

In the personal `tui` layout, `omp-agents` displays OMP's native task/eval children, Claude Code's native subagents, and Pi's foreground children beneath their parent, alongside real core child terminals. It reads the reporters' `omp.subagents`, `claude.subagents`, and `pi.subagents` labels through the App's `terminals()` API; it does not create dummy terminals or change agent execution. Each child shows its name and reported model. Unknown child models stay blank. Clicking a metadata row focuses the actual parent; only genuine core terminals support native peek. Fold controls include metadata children; the compact rail omits their rows. Metadata does not change the parent's status or priority.

Child branch connectors align beneath the center of the parent icon rather than beneath its text. Each metadata child occupies one line, with a small theme-colored circle instead of a visible status label. Full state and activity descriptions are available in the row's tooltip. Claude's stop-confirmation wait and unavailable transcript data use neutral dots rather than suggesting completion.

Snapshot keys include the runtime, exact machine-qualified parent address, session, and child ID, so local, remote, and cross-runtime children cannot collide. Expired snapshots disappear after fifteen seconds, even while idle. OMP's and Pi's settled child rows disappear after five seconds; Claude's stopped or confirmed terminal rows disappear after ten seconds. See [OMP Desktop reporting](../omp/README.md#herdr-desktop), [Claude Desktop reporting](../claude/README.md#herdr-desktop), and [Pi Desktop reporting](../pi/README.md#herdr-desktop) for lifecycle, storage, transport precedence, and runtime limitations. Codex remains TUI-only. The installed `omp-agents` mod name is retained so existing personal layouts need no migration.

Herdr App 0.1.15 does not support imports from another mod's source. The personal Agents mod therefore retains an explicit Apache-2.0-licensed copy of that release's Agents implementation and adds metadata projection in [snapshot.js](../herdr-app/files/mods/omp-agents/snapshot.js). Shipped files are not edited. Review upstream Agents changes when updating App versions; personal copies do not automatically inherit them.

## Agent detection

When both Pi and Herdr modules are enabled, the [Pi parent status integration](../pi/README.md#herdr-parent-status) reports working, idle, and blocked states through Herdr's bundled extension. Home Manager installs the version matched to the pinned Herdr package. Pi's model and subagent display metadata does not set the parent lifecycle state.

When OMP and Herdr are enabled, the [OMP integration](../omp/README.md#herdr-agents-sidebar) installs Herdr's bundled OMP extension for parent lifecycle and session identity. A separate local extension reports the parent model and native subagent metadata. OMP is detected as `omp`, not `pi`, and uses its own sidebar layout.

The packaged Herdr excludes direct `claude plugin ...`, `claude mcp ...`, `claude --version` / `-v`, and `claude --help` / `-h` invocations from process-based agent detection. Home Manager runs plugin and MCP management commands during `setupClaudeCode`; they do not represent an interactive Claude session. Normal Claude sessions, including resume and continue, remain detectable. Unrecognized argument layouts retain upstream detection behavior.

This behavior comes from [the local detection patch](../../packages/apps/herdr/claude-admin-detection.patch). After applying Home Manager, restart the Herdr server when its running tasks can be interrupted to load the patched binary. `herdr server reload-config` only reloads configuration.

## Subagents

Agent entries in [files/config.toml](files/config.toml) show the agent name and reported model on their second row. Codex reads the parent's latest `turn_context.model`, Pi reports its current session model, and [Claude's status line](../claude/README.md#settingsjson) reports its selected model. Claude's model expires after 45 seconds without a status line update and may disappear while idle. Missing model metadata is hidden; agents without a model reporter show only their name. Parent rows do not display summaries or child counts.

The Claude, Pi, and Codex layouts also display child names, models, and activity. The [Pi extension](../pi/README.md#herdr-subagent-sidebar) reports foreground children. The [Codex hook and watcher](../codex/README.md#herdr-subagent-sidebar) read direct child sessions from local rollout files. The [Claude hook and watcher](../claude/README.md#herdr-subagent-sidebar) read native child transcripts and parent results. Both watchers include subagents that continue in the background. Empty child rows are hidden.

Codex shows a colored status icon, name, and model on each child's first line, with an excerpt of its latest assistant message below it in the generated theme's secondary text color. The [local separator patch](../../packages/apps/herdr/sidebar-status-separator.patch) places one space after custom lifecycle and overflow icons, without a dot separator. Load this patch by applying Home Manager and restarting the Herdr server when its running tasks can be interrupted; config reload alone does not load binary changes. The excerpt is limited to 80 characters with an ellipsis when truncated; tool events preserve it, and task completion uses the final response when available. Fixed labels are English; messages retain their original language. Running, completed, interrupted, and failed are separate states. Terminal states remain visible for five seconds. Read failures show an unknown state and the last message. If the watcher dies or cannot reach Herdr, its last metadata expires within 15 seconds; expiration removes rows rather than generating an unknown icon.

The expanded TUI sidebar at desktop width supports seven children with two lines each. If more children are visible, it shows six plus an overflow count. Codex prioritizes running and unknown children before recent terminal states. The rows belong to the parent pane and do not create additional terminal panes. Collapsed and mobile layouts remain compact.

Claude uses the same two-line layout and status colors. Its `SubagentStop` hook is tentative: `○` remains until a parent result or task notification confirms completion, failure, or interruption. For Agent Teams, removal from a valid team configuration led by the current session also confirms interruption. Task descriptions replace agent types when an Agent result arrives, and messages update from child transcripts. Confirmed terminal states and stop-confirmation waits remain visible for ten seconds, and stale metadata expires within 15 seconds. Sidebar cleanup preserves worktrees.

OMP uses the same status colors and seven-child layout. It consumes native task/eval lifecycle and progress events, including background children, and displays their resolved models and recent activity. Delivered nested events are supported, but complete deeper-descendant delivery depends on OMP's public event API. Terminal rows remain for five seconds; metadata expires after fifteen seconds without refresh. A parent turn ending does not clear running children. See [OMP's integration and reload limits](../omp/README.md#herdr-agents-sidebar).

Apply Home Manager, run `herdr server reload-config`, and start a new Claude, Pi, Codex, or OMP session inside Herdr to load the integration.

## Shared JSONL reader

The Codex and Claude watchers use [files/jsonl-tail.ts](files/jsonl-tail.ts) for incremental JSONL reads. The reader buffers incomplete UTF-8 records, resets offsets after file replacement or truncation, and diagnoses malformed or oversized records without logging their contents. Each runtime module packages its watcher and this shared file with their repository-relative paths intact; the reader does not require the Herdr module to be enabled.

Run the shared reader and runtime regression tests with:

```bash
bun test modules/herdr/tests modules/codex/tests modules/claude/tests
```

## Apply changes

Herdr updates follow the [app package update CI](../../packages/README.md#updates-and-ci). Its local patches are checked before the source pin changes, and successful updates are committed to `main` after evaluation and changed-package builds. Pull those changes before applying Home Manager.

Apply Home Manager for the current environment to install the terminal config, for example `home-manager switch --flake .#devsisters-macbook`. Reload Ghostty with `Cmd+Shift+,`, then run `herdr server reload-config` to reload the running Herdr server's bindings and theme. Changes to Herdr's source configuration or shared theme exports take effect after applying Home Manager and reloading Herdr.

## Retired TUI plugins

Herdr Annotate and Auto Title are no longer packaged or linked. On previously configured hosts, unregister them once while the Herdr TUI server is running:

```bash
herdr plugin unlink annotate
herdr plugin unlink herdr.auto-title
```

Repeating these commands is safe. They remove registrations without deleting saved annotations, plugin configuration, or local plugin files. They do not stop already-running plugin processes or panes; let existing work finish before stopping the old TUI server. Unlink is intentionally not a Home Manager activation step because it fails when no TUI server is running.

The six Annotate prefix bindings and Home Manager's `~/.config/plannotator-tui/config.toml` link are removed. Herdr App/Core, the agent sidebar integrations, and the separate Claude/Codex Plannotator CLI integration remain unchanged.
