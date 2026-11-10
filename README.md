# config-flakes

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
| [flake.nix](flake.nix), `flake.lock` | Forwarding entry point and dependency pins; inputs live in [config/flake.nix](config/flake.nix) and outputs in [config/outputs.nix](config/outputs.nix) |

Application and hardware projects are maintained in separate private repositories:

| Repository | Local checkout |
| --- | --- |
| [gytkk/menu](https://github.com/gytkk/menu) | `~/menu` |
| [gytkk/trading-agent](https://github.com/gytkk/trading-agent) | `~/trading-agent` |
| [gytkk/keyboards](https://github.com/gytkk/keyboards) | `~/keyboards` |
| [gytkk/keycult-trackball](https://github.com/gytkk/keycult-trackball) | `~/keycult-trackball` |

Each repository owns its source, history, checks, and project instructions. This repository retains the configuration flake, shared agent resources, and their supporting scripts, documentation, and CI. OpenClaw's independent workspace instructions, skills, and runbooks remain separate; see [OpenClaw task routing](config/modules/openclaw/README.md#task-routing-across-repositories).

The GitHub repository is `gytkk/config-flakes`; the local configuration checkout remains `~/ws` to preserve installed generations' out-of-store links. Runtime paths still default to that location. Application settings belong in `config/modules/`; shared agent instructions and skills belong in `agent-core/`.

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
