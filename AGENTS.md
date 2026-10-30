# Repository instructions

This repository contains independent configuration and agent-resource areas. Follow global agent instructions and preserve unrelated work.

- Before changing `config/`, the root `flake.nix` or `flake.lock`, or configuration package workflows under `.github/`, read [config/AGENTS.md](config/AGENTS.md).
- Before changing shared agent rules, runtime adapters, skills, source selections, or renderer behavior, read [agent-core/README.md](agent-core/README.md). Canonical shared sources remain in `agent-core/`; runtime settings belong in `config/modules/`.
- Keep root `flake.nix` as the input and entry-point boundary, with configuration outputs in `config/outputs.nix`. Keep root `scripts/` for shared repository tools.
- Worktrees are allowed. Runtime checkout paths default to `~/ws`; before switching from a worktree, deliberately configure that path or apply from the main checkout.
- Use `config/modules`, `config/themes`, and `config/secrets` for configuration paths. Read [checkout migration](config/README.md#checkout-paths-and-migration) when applying or rolling back generations that use the former root paths.
- Give future application or workspace projects their own local instructions. Configuration-specific documentation restrictions and Nix checks apply to configuration work.
- Commit completed, validated task changes unless the user requests otherwise. Do not push unless explicitly requested.

For documentation changes, run `uv run --no-project scripts/check-paths.py` and pass other changed Markdown files explicitly when needed. See [README.md](README.md) for repository navigation.
