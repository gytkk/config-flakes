# Repository instructions

Before starting work in `config/`, read and follow [config/AGENTS.md](config/AGENTS.md) and any more specific instructions that apply.

This is the `gytkk/config-flakes` repository: configuration and shared coding-agent resources. Application and hardware projects have their own repositories and checkout instructions. Worktrees are allowed; preserve unrelated work.

- Before changing the root `flake.nix` or `flake.lock`, or configuration package workflows under `.github/`, read and follow [config/AGENTS.md](config/AGENTS.md).
- Before changing shared agent rules, runtime adapters, skills, source selections, or renderer behavior, read [agent-core/README.md](agent-core/README.md). Canonical shared sources remain in `agent-core/`; runtime settings belong in `config/modules/`.
- Keep root `flake.nix` as a forwarding entry point, configuration inputs in `config/flake.nix`, and configuration outputs in `config/outputs.nix`. Manage configuration dependency pins in the root `flake.lock`. Keep root `scripts/` for shared repository tools.
- Worktrees are allowed. Configuration runtime checkout paths default to `~/ws`; before applying Home Manager or NixOS configuration from a worktree, deliberately configure that path or apply from the main checkout.
- Use `config/modules`, `config/themes`, and `config/secrets` for configuration paths. Read [checkout migration](config/README.md#checkout-paths-and-migration) when applying or rolling back generations that use the former root paths.
- Application and hardware work belongs in the separate private repositories `gytkk/menu`, `gytkk/trading-agent`, `gytkk/keyboards`, and `gytkk/keycult-trackball`, normally checked out at `~/<repository-name>`. Read the target repository's instructions and run its checks there; configuration rules do not apply to those projects.
- Before changing OpenClaw resources or their paths, read [OpenClaw task routing](config/modules/openclaw/README.md#task-routing-across-repositories). Its independent workspace instructions, skills, and automation runbooks live outside this repository; shared coding-agent resources in `agent-core/` are not their replacement.
- Commit completed, validated task changes unless the user requests otherwise. Do not push unless explicitly requested.

For documentation changes, run `uv run --no-project scripts/check-paths.py` from the repository root. Pass other changed Markdown files explicitly when their code paths are relative to the repository root; the checker does not resolve project-relative code paths. For project documentation, also run the affected project's documentation checks from its project root. See [README.md](README.md) for repository navigation.
