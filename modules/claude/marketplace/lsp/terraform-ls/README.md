# terraform-ls

Terraform language server ([terraform-ls](https://github.com/hashicorp/terraform-ls)) plugin for Claude Code.

## Prerequisites

The `terraform-ls` binary must be available on your `PATH`. This repository's
[LSP module](../../../../lsp/default.nix) installs it when enabled; the plugin
runs `terraform-ls serve`.

## Supported file types

| Extension | Language |
|-----------|----------|
| `.tf` | Terraform |
| `.tfvars` | Terraform Vars |

## Usage

Home Manager registers this checkout's local `gytkk` marketplace and installs
`terraform-ls@gytkk` through the [Claude module](../../../README.md). Apply the
configuration and start a new Claude session.

For manual registration, run from this repository's root:

```bash
claude plugin marketplace add "$PWD/modules/claude/marketplace"
claude plugin install terraform-ls@gytkk
```
