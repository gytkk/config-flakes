# nix-flakes

This repository keeps system configuration and shared coding-agent resources in separate directories, leaving room for future application and workspace projects.

| Path | Responsibility |
| --- | --- |
| [config/](config/README.md) | Home Manager, NixOS, packages, secrets, and generated themes |
| [agent-core/](agent-core/README.md) | Shared agent rules, adapters, skills, and renderer |
| `scripts/` | Repository checks and agent skill installation |
| `flake.nix`, `flake.lock` | Root Nix inputs and entry point; outputs come from `config/outputs.nix` |

Run configuration commands from the repository root:

```bash
home-manager switch --flake .#<environment>
nixos-rebuild switch --flake .#<host>
```

See the [configuration guide](config/README.md) for prerequisites, environments, tools, and activation details. Runtime checkout paths default to `~/development/nix-flakes`. Worktrees are allowed for editing and validation; switching from one requires a deliberate checkout-path configuration or applying from the main checkout.

The temporary root `modules`, `themes`, and `secrets` symlinks preserve paths used by existing generations and stored marketplace registrations. Keep them until deployed generations, rollback-required generations, and stored paths have migrated. This layout does not imply that any host has switched. See [checkout migration](config/README.md#checkout-paths-and-migration).

No `app/` or `workspace/` project exists yet. Future projects can own their implementation and local instructions alongside `config/` and `agent-core/`.
