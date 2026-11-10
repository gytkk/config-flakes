# nixd-lsp

Nix language server ([nixd](https://github.com/nix-community/nixd)) plugin for Claude Code.

## Prerequisites

The `nixd` binary must be available on your `PATH`.

### Installation

```bash
# Nix (recommended)
nix-env -iA nixpkgs.nixd

# Nix profile
nix profile install nixpkgs#nixd
```

## Supported file types

| Extension | Language |
|-----------|----------|
| `.nix` | Nix |

## Usage

```bash
claude plugin marketplace add gytkk/claude-marketplace
claude plugin install nixd-lsp@gytkk
```
