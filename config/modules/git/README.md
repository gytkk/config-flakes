# Git Module

This module provides comprehensive Git configuration with sensible defaults and productivity enhancements.

## What it does

- Configures Git with user identity and email
- Enables Git LFS (Large File Storage) support
- Sets up custom configuration for diff, pull, push, and color settings
- Creates a global `.gitignore` file with common patterns
- Configures Neovim as the default editor and Delta for Git diff output

## Features

- **User Configuration**: Sets username to `gytkk` and email to `gytk.kim@gmail.com`
- **Git LFS**: Automatically handles large files
- **Smart Defaults**:
  - Rebase on pull with fast-forward only
  - Auto-setup remote for push
  - Default branch set to `main`
  - Colorized output for better readability
- **Global Gitignore**: Excludes common files like `.DS_Store`, `.idea`, `.vscode`, `.env`, etc.

## Requirements

- Nix package manager
- Home Manager
- Neovim (for the editor)

## Configuration Details

### Git Settings

- **Editor**: Neovim (`nvim`)
- **Diff display**: Delta with side-by-side output, line numbers, the `ansi` syntax theme using the terminal palette, and light diff backgrounds. Git's own diff colors use terminal red and green.
- **Pull Strategy**: Rebase with fast-forward only
- **Push Strategy**: Current branch with auto-setup of remote
- **Default Branch**: `main`

### Global Gitignore Patterns

- System files: `.DS_Store`
- IDE files: `.idea`, `.vscode`
- Environment files: `.env`, `.envrc`, `.tool-versions`
- Python files: `.coverage`

## Usage

After applying this module, Git will be configured with all the settings. You can verify the configuration with:

```bash
git config --list
```

The global gitignore will be applied automatically to all repositories.
