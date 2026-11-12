# ty-lsp

Python type checker and language server ([ty](https://github.com/astral-sh/ty) by Astral) plugin for Claude Code.

## Prerequisites

The `ty` binary must be available on your `PATH`. This repository's
[LSP module](../../../../lsp/default.nix) installs it when enabled; the plugin
runs `ty server`.

## Supported file types

| Extension | Language |
|-----------|----------|
| `.py` | Python |
| `.pyi` | Python |

## Usage

Home Manager registers this checkout's local `gytkk` marketplace and installs
`ty-lsp@gytkk` through the [Claude module](../../../README.md). The managed
Claude settings disable `pyright-lsp@claude-plugins-official` so Python requests
use ty. Apply the configuration and start a new Claude session.

For manual registration, run from this repository's root:

```bash
claude plugin marketplace add "$PWD/modules/claude/marketplace"
claude plugin install ty-lsp@gytkk
```
