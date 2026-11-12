# nixd-lsp

Nix language server ([nixd](https://github.com/nix-community/nixd)) plugin for Claude Code.

## Prerequisites

The `nixd` binary must be available on your `PATH`. This repository's
[LSP module](../../../../lsp/default.nix) installs it when enabled; no separate
`nix-env` or profile installation is needed.

## Supported file types

| Extension | Language |
|-----------|----------|
| `.nix` | Nix |

## Usage

Home Manager registers this checkout's local `gytkk` marketplace and installs
`nixd-lsp@gytkk` through the [Claude module](../../../README.md). Apply the
configuration and start a new Claude session.

For manual registration, run from this repository's root:

```bash
claude plugin marketplace add "$PWD/modules/claude/marketplace"
claude plugin install nixd-lsp@gytkk
```
