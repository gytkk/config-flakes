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

Interactive Zsh prints total elapsed time after foreground `home-manager` and `nixos-rebuild` commands, including the `hm`, `hmb`, and `hms` aliases and commands prefixed with `sudo`. For example, `[home-manager] 소요 시간: 2분 13초` appears on stderr when the next prompt is displayed, including after a failed command. The [timing hooks](nix-command-timing.zsh) inspect the expanded command without wrapping its executable or changing arguments, output streams, or exit status.

This measures the whole command line, including sudo authentication and any subsequent pipeline or chained commands. It does not divide evaluation, builds, and activation into stages. Background jobs, commands embedded in scripts or compound shell constructs, and noninteractive shells are outside this measurement. After applying the module, run `exec zsh -l` to load the hooks. To measure the first switch before applying them, run `source modules/zsh/nix-command-timing.zsh` from the checkout in the current Zsh session.

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
