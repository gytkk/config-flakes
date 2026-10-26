# Zsh module

This module configures Zsh, a Starship prompt, shell aliases, and tool integrations through Home Manager. `base/default.nix` enables it by default.

## What it does

- Installs Zsh and configures its startup files under `~/.config/zsh`
- Configures Starship from the generated theme selected by `modules.commonTheme`
- Enables syntax highlighting, autosuggestion, and completion
- Provides development-focused aliases and tools integration
- Integrates fzf (fuzzy finder) and direnv for enhanced workflow

## Features

### Shell enhancements

- **Syntax Highlighting**: Real-time syntax highlighting for commands
- **Autosuggestion**: Intelligent command suggestions based on history
- **Tab Completion**: Enhanced completion system
- **History Management**: 10,000 command history with deduplication and sharing

### Theme and appearance

- **Starship**: `~/.config/starship.toml` links to the selected `themes/exports/starship/<theme-id>.toml` in the checkout
- **Color Support**: Colorized ls output and completion menus

### Development aliases

- **Editor**: `vim` and `vi` run `nvim`; `vimdiff` runs `nvim -d`
- **File Listing**: `ll`, `lh` with colors
- **Kubernetes**: `kl` (kubectl), `kx` (kubectx), `kn` (kubens)
- **Home Manager**: `hm`, `hmb`, and `hms` run Home Manager, build, and switch
- **Git**: Common aliases such as `gst`, `gsw`, and `gd`

`gsww <branch>` removes another worktree holding the branch, then switches the current worktree to that branch. If removal fails, it stops without switching.

The [Terraform module](../terraform/README.md) owns the optional `tf` alias and its `runEnv` variables.

### Nix command timing

Interactive Zsh wraps `home-manager` and `nixos-rebuild`, including `hm`, `hmb`, and `hms`, with the [timing runner](nix-command-timing.py). The [shell functions](nix-command-timing.zsh) also recognize these commands after common `sudo` options and pass the original arguments to the real sudo executable. Other sudo commands run directly. After the command exits, a table on stderr reports each detected stage, the total, and the exit code. Durations use minutes and seconds to one decimal place, for example `2분 13.4초`. Command exit codes and stdin are preserved; stdout and stderr are streamed separately. Because the child output uses pipes, terminal-only progress displays or colors may be disabled. Sudo still uses the original stdin and controlling terminal for authentication.

Home Manager's initial interval includes preparation, evaluation, downloads, and builds. `Starting Home Manager activation` and each `Activating <name>` message start separate rows, so `setupClaudeCode` and `installAgentSkills` appear individually. NixOS uses its build, service-stop, system-activation, service-application, and completion messages. NixOS Home Manager activation normally logs to the system journal, so its duration is included in service application rather than split into individual activation rows.

These are elapsed intervals between received log markers, not internal profiler measurements. Concurrent build and download work is not split, buffered messages can delay a boundary, and cleanup after the last marker is included in the final stage. Unrecognized or translated messages do not create boundaries; a command with no recognized markers reports only its total. Only the wrapped command is measured, including sudo authentication when applicable, rather than subsequent pipeline or chained commands. No extra evaluation or build is run for measurement.

After applying the module, run `exec zsh -l`. To bypass timing, use `command home-manager ...`, `command nixos-rebuild ...`, or `command sudo nixos-rebuild ...`. Scripts can opt in with `nix-command-timing --tool home-manager -- home-manager switch ...`. To measure the first switch before the runner is installed, run `uv run --no-project modules/zsh/nix-command-timing.py --tool home-manager -- home-manager switch --flake .#<environment>` from the checkout. Run isolated parser, stream, and shell tests with `uv run --no-project -m unittest discover -s modules/zsh/tests -v`; they do not switch a real configuration.

### Tool integration

- **fzf**: Fuzzy finder with Zsh integration
- **zoxide**: Directory jumping with Zsh integration
- **direnv**: Directory-based environment variable management
- **uv**: Python package manager with shell completion

## Requirements

- Nix package manager
- Home Manager
- Zsh shell support

## Nix executable priority on macOS

Login shells put `~/.nix-profile/bin` first in `PATH` through the Home Manager-generated `.zprofile`, after macOS runs `path_helper`. This makes commands such as `python3` use the Nix installation. Non-login child shells inherit that order, and activating a virtual environment can still override Python for the current shell.

After applying Home Manager, start a new login shell with `exec zsh -l`. Restart applications such as Codex from that shell so they inherit the updated `PATH`.

## Configuration files

- `~/.config/zsh/`: Home Manager-generated startup files
- `~/.config/starship.toml`: Link to the generated canonical theme export
- `~/.zsh_history`: Command history storage

## Usage

After applying this module, restart your shell or run:

```bash
exec zsh -l
```

The configuration will automatically:

- Load the selected Starship theme
- Enable completion, syntax highlighting, and autosuggestions
- Set up aliases and integrations
- Configure optimal shell settings
