# Vim module

This module configures the pinned Neovim 0.13 development build with LazyVim and personal Lua settings managed through Home Manager. It installs Neovim as the default editor and creates `vi` and `vim` aliases.

## Configuration

Home Manager links `nvim/lua/config` and `nvim/lua/plugins` separately to the corresponding directories under `modules/vim/files/lua/`. The generated `init.lua` supplies the shared theme, decrypted agenix key path, and `vim.g.nix_flakes_lockfile`, then loads `config.lazy`. LazyVim provides common plugin setup; personal specs preserve the selected language servers, formatting, keymaps, search refresh, and startup behavior. Mason, Dashboard, and Bufferline are disabled.

LazyVim is pinned to `16.0.1`. Plugin revisions are recorded in the [repository lockfile](files/lazy-lock.json), which Lazy uses directly. An existing unmanaged `~/.config/nvim/lazy-lock.json` is left untouched and is ignored. Run `:Lazy restore` to restore recorded revisions. `:Lazy update` modifies the repository lockfile, so review and commit its diff. Configure reproducible extras in repository specs; interactive `:LazyExtras` selections are not managed here.

The module exposes every generated colorscheme from `themes/exports/nvim/`, loads the selected shared theme, and falls back to `one-half-light` if its export is missing. Vira Graphene follows the original editor's syntax and UI colors through a theme override, with comments brightened to `#676767` and selection set to `#545454`. Neovim cannot alpha-blend a Visual highlight background. See the [canonical theme resolution rules](../../themes/README.md#resolution-rules).

## Features

- Snacks provides file picking, explorer, notifications, status column, inlay hint toggles, lazygit, git browse, file rename, inline images, and GitHub issues and pull requests through `gh`. Scroll animation is disabled by default; `<leader>uS` toggles it for the current session.
- Persistence saves directory and branch-aware sessions and restores them on bare `nvim` startup.
- Blink combines LSP, snippets, paths, and buffers in a completion menu. Minuet displays automatic AI suggestions as dimmed inline text.
- Nix installs language servers; Rust uses rust-analyzer from the active rustup toolchain. Neovim watches Rust files to avoid a macOS server watcher startup stall.
- Treesitter provides syntax parsing. Conform formats with the selected tools, including `nixfmt`, `prettier`, `rustfmt`, `gofmt`, and `ruff_format`. Markdown and YAML skip formatting on save; manual formatting remains available.
- Native diagnostics show a color-coded circle and the current-line message directly after the code.
- Markdown and MDX use `render-markdown.nvim` for an in-buffer preview that returns to source in insert mode. Directly opened standalone Markdown files receive Marksman support.

Flash is pinned to the upstream Neovim 0.13 search-state fix. Older versions access removed `search_match_*` globals and can repeatedly report `Flash error during redraw` after a character motion, including when saving MDX files. Restore the recorded plugins after applying this configuration and restart Neovim.

## Root and editing defaults

Trailing spaces display as blanks in every filetype. Tabs and non-breaking spaces retain their visible markers.

LazyVim detects the project root from the attached LSP, then `.git` or `lua` markers, and finally the working directory. File search, grep, and `<leader>e` use that root. The startup explorer, `<leader>fF`, `<leader>sG`, and `<leader>E` use the working directory instead. `:LazyRoot` shows the detected roots. Grep still saves the current buffer before searching through `<leader>/`, `<leader>sg`, or `<leader>sw`, and refreshes results after file changes.

Editing options inherit LazyVim defaults, including automatic writes during eligible buffer switches and commands, relative line numbers, visible whitespace, current-line highlighting, and a global statusline. Page motions such as `<C-f>`, `<C-b>`, `<C-d>`, and `<C-u>` scroll immediately without animation. Markdown, MDX, and YAML explicitly enable `wrap` and `linebreak` when their filetype is set or they enter a window, including during session restoration, so returning to a document restores wrapping even if it was toggled off. This wraps display lines without inserting newlines into the file. Other code buffers do not wrap. Spell checking is disabled in Markdown and MDX to avoid marking Korean text as misspelled. Other text filetypes retain LazyVim's wrapping and spell-checking defaults. Folding uses indentation or available Treesitter/LSP support. New buffers and restored sessions start with all folds open; manual folding remains available. `<leader>` is `<Space>` and `<localleader>` is `\`. The Markdown/YAML automatic formatting exclusions remain separate from automatic writes.

## Search refresh

For `/` and `?` searches, entering insert mode, changing buffer text, leaving a buffer, or reloading an externally changed file clears search highlights and Noice's inline search count. Press `<Esc>` in normal mode to clear them manually. The pattern remains available, and `n` or `N` resumes the search with highlights and the count.

Open grep pickers rerun the current search after a file is saved, after Neovim reloads an externally changed file, and when Neovim regains focus. Results and preview refresh while preserving the query and scope, including a picker reopened with `<leader>sR`. External edits appear when Neovim detects the change or regains focus; there is no continuous filesystem polling.

## Blink and Minuet

Minuet requests OpenAI `gpt-6-luna` suggestions with no reasoning and displays them automatically as inline text for the configured coding filetypes, including Nix. The text uses the theme's `Comment` highlight and can appear while Blink's ordinary completion menu is open. Minuet loads before filetype detection so automatic suggestions also work in the first opened file. AI requests use only Minuet's inline frontend; Blink's own ghost text is disabled.

- `<Tab>` accepts a visible AI suggestion and closes the Blink menu. Without an AI suggestion, it follows Blink's `super-tab` rules: accept a menu item, advance an active snippet, or fall back to ordinary Tab behavior.
- `<C-y>` requests an inline suggestion manually when none is available, bypassing the automatic filetype restriction and throttle.
- `<Esc>` leaves insert mode and clears the inline suggestion.
- Minuet uses a 6,000-character context window, a 900ms throttle, a 180ms debounce, and a 4-second timeout.
- Requests use OpenAI Fast mode at its Fast mode token rate, with a 1,024-token limit shared by reasoning and visible output. Automatic requests can run while an LSP menu is open. Actual API quality, latency, and usage require live verification.

Store the API key in `secrets/openai-api-key.age`. NixOS decrypts it under `/run/agenix/openai-api-key`; home-only environments use Home Manager agenix through the [OpenAI API module](../openai-api/README.md). The vim module supplies the decrypted path as `vim.g.openai_api_key_path`.

```bash
EDITOR=vim agx -e openai-api-key.age
```

## Notable keymaps

`<leader>` is `<Space>`. LazyVim supplies the search, rename, implementation, Trouble, Git hunk, toggle, and manual formatting defaults below. Personal search refresh hooks and completion remain configured separately. Lualine extends the default statusline with Minuet status.

- `<leader>e`, `<leader>E`: file explorer at the project root or working directory
- `<leader>ff`, `<leader>fF`: find files at the project root or working directory
- `<leader>fb`, `<leader>,`: buffer list
- `H`, `L`: Vim's native screen-top and screen-bottom motions, respecting the two-line `scrolloff` margin; `[b` and `]b` switch buffers
- `<leader>sg`: grep at the project root, saving the current modified file and checking external changes first
- `<leader>sG`: grep in the working directory
- `<leader>sR`: resume the previous picker with its query and scope
- `<leader>sr`: search and replace
- `<leader>sb`: search lines in the current buffer
- `<leader>s/`: search history
- `<leader>gs`, `<leader>gb`, `<leader>gd`, `<leader>gS`: git status, branches, diff, and stash pickers
- `<leader>gf`, `<leader>gl`, `<leader>gL`: git log for current file, repository, and current line
- `<leader>gg`: lazygit
- `<leader>gB`: open current file or selection in the git remote browser
- `<leader>ghs`, `<leader>ghr`: stage or reset the current hunk or visual selection
- `<leader>ghp`: preview the current hunk inline
- `<leader>ghb`: blame current line
- `]h`, `[h`, `]H`, `[H`: next, previous, last, and first Git hunk
- `<leader>gi`, `<leader>gp`: GitHub issues and pull requests
- `<leader>qs`, `<leader>qS`, `<leader>ql`: restore directory session, select a session, and restore the last session
- `<leader>qd`: stop saving the session on exit
- `gd`: go to definition
- `gI`: go to implementation; `gi` uses Vim's native last-insert-position behavior
- `gb`: jump back in the jumplist
- `<leader>cr`: rename symbol
- `<leader>cR`: rename current file with LSP-aware updates
- `<leader>cl`: LSP information
- `<leader>cS`: LSP references and definitions in Trouble
- `<leader>ud`: toggle diagnostics
- `[d`, `]d`: previous and next diagnostic
- `<leader>d`: toggle all diagnostics on the current line
- `<leader>xX`, `<leader>xx`: current-buffer and workspace diagnostics in Trouble
- `<leader>uh`, `<leader>ul`, `<leader>uS`, `<leader>uw`: toggle inlay hints, line numbers, smooth scrolling, and wrap
- `<leader>cf`: format the buffer or visual selection through LazyVim, including Markdown and YAML with automatic formatting disabled

## Usage and rollout

Bare `nvim`, `vim`, or `vi` restores only the current working directory's session, preferring the current Git branch and falling back to that directory's unbranched session. It opens the left explorer at the working directory after all `VeryLazy` handlers finish, even if a restored file or symlink belongs to another project. Without a matching session, it opens only the explorer. LazyVim's default and personal autocmds are registered before restoration opens files, so their normal file and window events apply settings across restored tabs and splits. Opening a file with `nvim filename.txt`, `vim filename.txt`, or `vi filename.txt` skips startup session restore and explorer opening. The default setup uses the existing `nvim` session state. Sessions from the former isolated candidate are not copied.

Before applying the promoted setup, close Neovim. If rollback is needed, first save the existing config lockfile and Neovim data, including sessions, outside their active directories. Apply Home Manager for the target environment, restore the repository's plugin revisions, and restart Neovim:

```bash
home-manager switch --flake .#pylv-denim
nvim --headless '+Lazy! restore' +qa
nvim
```

Use the corresponding `nixos-rebuild switch --flake .#<host>` procedure on NixOS. The first restore may download plugins and install parsers; the module installs the required `tree-sitter` CLI. Applying Home Manager or rebuilding the host is a user-run step.

## Regression checks

Check folding defaults without loading plugins or touching saved sessions:

```bash
nvim --headless -u NONE -i NONE -l modules/vim/tests/folds.lua
```

The completion check uses the real merged LazyVim, Blink, and Minuet specs with a substituted backend, preventing external requests and credential access. It covers automatic Nix inline suggestions, preview rendering, Tab acceptance, ordinary completion and snippet fallbacks, and filetype filtering. After plugins are installed, run it from the repository root:

```bash
LAZYVIM_SOURCE="${XDG_DATA_HOME:-$HOME/.local/share}/nvim/lazy/LazyVim" \
NVIM_PLUGIN_ROOT="${XDG_DATA_HOME:-$HOME/.local/share}/nvim/lazy" \
nvim --headless -u NONE -l modules/vim/tests/completion.lua

nvim --headless modules/vim/README.md \
  '+lua dofile("modules/vim/tests/startup.lua")'
```

The startup check covers theme loading, the initial Markdown buffer and Marksman attachment, Nix ownership of server installation, default options and keymaps, project-root versus working-directory search scopes, Minuet in the default statusline, diagnostics, and automatic versus manual formatting. Wait for parser installation before running it. Its Git keymap checks require a tracked file; it can also run against a tracked MDX file. In headless mode the test supplies the terminal's `UIEnter` event.

The session check attaches an embedded UI to a child Neovim with no file arguments, using installed plugins and a temporary workspace and state directory. It checks that LazyVim setup precedes restoration, filetype hooks run, and Markdown/MDX/YAML settings apply across restored tabs and splits, including a subsequent manual restore. It also verifies that restoring a symlink into another project keeps the startup explorer at the working directory. Existing saved sessions are not used or modified.

```bash
nvim --headless -u NONE -i NONE -l modules/vim/tests/session.lua
```

For checks that must preserve normal session and plugin state, use a temporary `NVIM_APPNAME` with a config directory linked to the installed config and isolated data, state, and cache. Install the recorded plugins in that runtime before running startup checks. There is no permanent candidate launcher.

Real editing checks still cover startup with and without sessions, file arguments, search refresh after saves and external edits, AI acceptance, and Rust watchers on macOS. Offline checks do not establish live API behavior or platform-specific UI behavior.

## Files

- [default.nix](default.nix): Home Manager wiring and generated initialization
- `modules/vim/files/lua/config/`: LazyVim bootstrap, options, autocmds, keymaps, AI credentials, theme loading, and session helpers
- `modules/vim/files/lua/plugins/`: personal plugin specs grouped by feature
- [lazy-lock.json](files/lazy-lock.json): recorded plugin revisions
- `themes/exports/nvim/`: generated colorschemes

## Historical investigation

The 2026-10-27 investigation compared the hand-maintained setup at `0080d76222accbafbfaf2bc08e1c15241d40ae8e` with LazyVim. Most core plugins were already shared, so the expected benefit was reduced ownership of common configuration. An isolated candidate tested the personal search, session, language-server, formatting, theme, and completion behavior before promotion to the default configuration.

The same investigation found that Minuet's inline frontend and Blink integration shared OpenAI backend cancellation state. Offline checks showed that opening the Blink menu did not clear inline text and an in-flight response could redisplay it after dismissal. The initial promotion used Blink for AI requests and presentation. The current setup uses only the inline frontend for AI requests, allowing inline suggestions alongside Blink's ordinary completion menu. Evidence: [Minuet virtualtext](https://github.com/milanglacier/minuet-ai.nvim/blob/ee03326381c93f6417fa66aeb2ca85a26c0706e7/lua/minuet/virtualtext.lua), [shared OpenAI backend](https://github.com/milanglacier/minuet-ai.nvim/blob/ee03326381c93f6417fa66aeb2ca85a26c0706e7/lua/minuet/backends/openai_base.lua), and [LazyVim Blink configuration](https://github.com/LazyVim/LazyVim/blob/999700997f72227187d49d8b92667183dc7fc809/lua/lazyvim/plugins/extras/coding/blink.lua).
