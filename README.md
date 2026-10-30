# ws

This repository manages Home Manager and NixOS configuration alongside shared coding-agent rules and skills. System configuration lives in `config/`; shared agent resources live in `agent-core/`.

| Path | Responsibility |
| --- | --- |
| [config/](config/README.md) | Home Manager, NixOS, packages, secrets, and generated themes |
| [config/inventory.nix](config/inventory.nix) | Environment names, platforms, users, and profiles |
| [config/modules/](config/modules) | Application settings and Home Manager or NixOS integration |
| [config/packages/](config/packages/README.md) | Package definitions, update tools, and checks |
| [config/themes/](config/themes/README.md) | Shared theme definitions, templates, and generated exports |
| [config/secrets/](config/secrets) | Agenix-encrypted secrets and recipient rules |
| [agent-core/](agent-core/README.md) | Shared agent rules, adapters, skills, and renderer |
| [scripts/](scripts) | Documentation path checks and agent skill installation |
| [flake.nix](flake.nix), `flake.lock` | Nix inputs and entry point; outputs come from [config/outputs.nix](config/outputs.nix) |

Runtime paths default to `~/ws`. Some installed settings link directly to files in this checkout, so its location matters after activation. Application settings belong in `config/modules/`; shared agent instructions and skills belong in `agent-core/`.

Run configuration commands from the repository root, using an environment or host name from [the inventory](config/inventory.nix):

```bash
home-manager switch --flake .#<environment>
sudo nixos-rebuild switch --flake .#<host>
```

See the [configuration guide](config/README.md) for prerequisites and activation details, and the [agent-core guide](agent-core/README.md) for shared rules, skill installation, and validation. Worktrees can be used for editing and validation; switching from one requires configuring its runtime checkout path or applying from the main checkout.

The root has no `modules`, `themes`, or `secrets` compatibility links. Hosts and marketplace registrations must use the paths under `config/`. Before rolling back to a generation that uses the former paths, follow [checkout migration](config/README.md#checkout-paths-and-migration).

After editing documentation, check local links and repository paths:

```bash
uv run --no-project scripts/check-paths.py
```
