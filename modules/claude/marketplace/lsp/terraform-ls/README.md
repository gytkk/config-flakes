# terraform-ls

Terraform language server ([terraform-ls](https://github.com/hashicorp/terraform-ls)) plugin for Claude Code.

## Prerequisites

The `terraform-ls` binary must be available on your `PATH`.

### Installation

```bash
# macOS (Homebrew)
brew install hashicorp/tap/terraform-ls

# Nix
nix-env -iA nixpkgs.terraform-ls

# Manual
# Download from https://github.com/hashicorp/terraform-ls/releases
```

## Supported file types

| Extension | Language |
|-----------|----------|
| `.tf` | Terraform |
| `.tfvars` | Terraform Vars |

## Usage

```bash
claude plugin marketplace add gytkk/claude-marketplace
claude plugin install terraform-ls@gytkk
```
