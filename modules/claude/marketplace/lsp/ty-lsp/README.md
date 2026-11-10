# ty-lsp

Python type checker and language server ([ty](https://github.com/astral-sh/ty) by Astral) plugin for Claude Code.

## Prerequisites

The `ty` binary must be available on your `PATH`.

### Installation

```bash
# uv (recommended)
uv tool install ty

# pip
pip install ty

# Nix
nix-env -iA nixpkgs.ty
```

## Supported file types

| Extension | Language |
|-----------|----------|
| `.py` | Python |
| `.pyi` | Python |

## Usage

```bash
claude plugin marketplace add gytkk/claude-marketplace
claude plugin install ty-lsp@gytkk
```
