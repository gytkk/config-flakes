# Git Module

Home Manager configures Git, Git LFS, Delta, and `~/.gitignore` when `modules.git.enable` is enabled.

## Configuration Details

### Git Settings

- **Editor**: Neovim (`nvim`)
- **Diff display**: Delta with side-by-side output, line numbers, the `ansi` syntax theme using the terminal palette, and light diff backgrounds. Git's own diff colors use terminal red and green.
- **Pull Strategy**: Rebase with fast-forward only
- **Push Strategy**: Current branch with auto-setup of remote
- **Default Branch**: `main`
- **Fetch**: Fetch all remotes and prune deleted branches and tags
- **GitHub credentials**: GitHub and Gist use `gh auth git-credential`; authenticate with GitHub CLI separately

### Global Gitignore Patterns

- System files: `.DS_Store`
- IDE and assistant files: `.idea`, `.vscode`, `.zed`, `.ai`, `.claude/settings.local.json`, `.sisyphus/`
- Environment files: `.direnv/`, `.env`, `.envrc`, `.tool-versions`, `mise.toml`
- Python caches, build output, package metadata, and coverage output
- Node.js logs
- Nix build results: `result`, `result-*`

