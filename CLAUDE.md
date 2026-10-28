# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> **Precedence**: General development rules (commit workflow, security, testing, etc.) are defined in the global `~/.claude/CLAUDE.md`. This file contains project-specific rules and reference documentation. **Project-specific rules take precedence** over global rules when they conflict.

## Operational Rules

### Critical Rules

- DO NOT use git worktree for this repository
- Follow existing code patterns and module structure in this repository
- Use `nixfmt` to format all Nix files before committing (delegate to subagent using sonnet model)
- Prefer narrow, fast checks. Run time-consuming Nix evaluation commands such as `nix eval` and `nix flake check --no-build` only when they are required to validate the requested change or the user explicitly requests them. Skip them for documentation-only changes, simple edits, and checks unrelated to the changed behavior.
- Skip Nix package build tests by default because they are expensive. Run `nix build`, `nix-build`, or `nix flake check` without `--no-build` only when the user explicitly requests a build or a concrete build-specific concern cannot be checked with formatting, static checks, targeted tests, or evaluation. A package change alone does not justify a build.
- When changing the canonical theme pipeline or generated theme exports, leave a local git commit in a sensible rollbackable unit before finishing the work
- Do NOT push unless explicitly requested

### Documentation Guidelines

- All documentation belongs in: AGENTS.md, CLAUDE.md, README.md, code comments (sparingly), commit messages
- When module paths or behavior change, update the module README and its root documentation links. Run `uv run --no-project scripts/check-paths.py` to check the current entry-point documents; pass other changed Markdown files explicitly.

### Build/Test/Lint Commands

**Agent-safe commands** (run only as needed under the validation rules above):

```bash
nixfmt <file.nix>                  # Format Nix files
nix flake show                     # Show available flake outputs
nix flake check --no-build         # Validate complex or evaluation-sensitive changes
nix eval .#homeConfigurations.pylv-denim.config.home.packages --apply 'x: map (p: p.name) x'
```

**User-run commands** (ask the user to run these):

Standalone Home Manager commands do not require `--impure`, but several
modules still install out-of-store symlinks to the checkout path.

```bash
home-manager switch --flake .#<environment>
nixos-rebuild switch --flake .#<host>
```

---

## Reference

### Nix Code Style

- **Formatting**: Enforced by `nixfmt` (2-space indent). Use `with pkgs;` for package lists.
- **Naming**: Variables/options use camelCase, files use lowercase, directories use lowercase/kebab, environments use kebab-case.
- **Module pattern**: Home Manager modules expose `options.modules.<name>.enable` and gate runtime config with `lib.mkIf cfg.enable`. `base/default.nix` owns common default enable values; profile files override with `lib.mkForce` or plain assignments when needed.
- **Imports**: Use relative paths, import directories by name (e.g., `../modules/claude`)
- **Conditionals**: `lib.mkIf`, `lib.mkMerge`, `lib.mkForce`
- **Host definition**: See `inventory.nix` for required fields (`kind`, `system`, `username`, `homeDirectory`, `profile`)
- **Secrets**: Use [agenix](https://github.com/ryantm/agenix). Run `agenix -e secrets/<name>.age` with the secret's filename; NixOS decrypts it to `/run/agenix/<secretName>`.

### Architecture

Nix flake configuration for standalone Home Manager and NixOS environments on macOS and Linux.

```text
flake.nix                         # Main flake configuration
inventory.nix                     # All Home Manager environments and NixOS hosts
base/default.nix                  # Common Home Manager configuration
base/<profile>/home.nix           # Profile-specific Home Manager extensions
agent-core/                       # Canonical agent rules, adapters, skills, and renderer
modules/<name>/default.nix        # Reusable Home Manager or NixOS module
modules/nixos/                    # Common NixOS modules and shared secrets
hosts/<name>/configuration.nix    # NixOS host configuration
packages/                        # Package catalog, shared helpers, updaters, and tests
packages/apps/                   # Non-nixpkgs app packages
lib/pkgs.nix                      # Overlay and per-system package-set construction
lib/home-configurations.nix       # Home Manager configuration builder
lib/nixos-configurations.nix      # NixOS configuration builder
lib/builders.nix                  # Backward-compatible builder aggregation
overlays/default.nix              # nixpkgs version overlays
secrets/secrets.nix               # Agenix recipient configuration
```

#### Environments

Defined in `inventory.nix` (single source of truth). `kind` field determines build type:

- **devsisters-macbook / devsisters-macstudio**: ARM64 macOS, devsisters base, home-only
- **pylv-denim**: x86_64 Linux/WSL, pylv base, home-only
- **pylv-sepia**: x86_64 Linux/NixOS server, pylv base (with Disko, agenix, copyparty)
- **pylv-onyx**: x86_64 Linux/NixOS, pylv base (with niri, DankMaterialShell, user-managed OpenClaw)

#### Base System

1. **`base/default.nix`**: Common config — core modules, standard dev packages, programs
2. **`base/devsisters/home.nix`**: saml2aws, vault, kc2aws, wg-cli, scala, ruby, databricks-cli, pantry, custom scripts
3. **`base/pylv/home.nix`**: Minimal (inherits base)
4. **`base/pylv/sepia.nix`**: Home Manager additions for `pylv-sepia`; system configuration lives in `hosts/pylv-sepia/configuration.nix`

### Module System

Each module in `modules/` manages a specific tool. **When modifying settings for any tool, look in the corresponding module directory first.** `agent-core/` owns canonical agent instructions and shared skills. Runtime modules consume generated outputs and must not reimplement rendering or selection.

Common NixOS modules live under `modules/nixos`; host-specific NixOS input modules and values live in `hosts/<name>/configuration.nix`. `modules/openclaw` keeps the NixOS and Home Manager integration for the user-managed OpenClaw install. It does not own the OpenClaw package, mutable state, or user service.

```text
modules/<name>/
├── default.nix    # Main module configuration (ALWAYS exists)
├── files/         # Static config files (JSON, TOML, etc.)
├── themes/        # Legacy or app-local themes; generated themes live in themes/exports/
└── agents/        # AI agent definitions (for AI tools)
```

#### Module reference

Before changing an app module, read the shared [module reference](AGENTS.md#module-reference) and [editor-specific notes](AGENTS.md#editor-specific-notes) for its source paths, installation behavior, and activation constraints. Those sections also cover VSCode's disabled status, Terraform's version management, and links to module READMEs.

### AI Coding Agent Notes

AI 코딩 에이전트 설정을 변경할 때 공통 지침, runtime adapter, shared skill은 `agent-core/`에서 수정한다. Runtime 전용 설정은 대응 모듈에서 수정한다.

- `agent-core/rules/`는 공통 지침, `agent-core/adapters/`는 runtime별 지침, `agent-core/skills/`는 로컬 skill과 전환 중인 legacy source를 관리한다. `agent-core/manifest.toml`이 문서 조합, 로컬 및 외부 skill source와 설치 target을 정의한다.
- Nix는 지침을 설치한다. [Agent skills module](modules/agent-skills/README.md)은 Home Manager switch마다 활성 Codex, Claude, Pi의 skill 설치와 갱신을 `scripts/install-agent-skills.py`와 pinned Skills CLI로 실행한다. 최초 전환의 소유권 충돌, 복구 또는 host discovery를 다룰 때는 [Agent core README](agent-core/README.md)를 읽는다. 모든 host의 코딩 에이전트 discovery 검증 전에는 해당 runtime의 legacy renderer와 vendored source를 정리하지 않는다.
- OpenClaw의 지침, skill, plugin, 세션 기록은 코딩 에이전트와 독립적으로 관리한다. 기존 호스트에서 공통 연동을 해제할 때는 [OpenClaw README](modules/openclaw/README.md#removing-the-shared-agent-integration)를 읽는다.
- Skill은 필요한 capability를 runtime 중립적으로 표현한다. Runtime tool, SDK, metadata, plugin 설정은 대응 module이 소유하며, runtime module에 shared skill 복사본이나 별도 selection logic을 추가하지 않는다.
- **Claude Code** (`modules/claude/`): Plugin과 marketplace 목록은 [Claude README](modules/claude/README.md)에서 확인한다. Runtime 전용 `devils-advocate`와 LSP plugin은 `modules/claude/marketplace/`에 있으며 LSP plugin은 `modules/lsp/default.nix`의 바이너리가 필요하다.

### Package Management

- **Base packages** (`base/default.nix`): nixfmt, coreutils, findutils, docker, gcc, jq, fd, ripgrep, git, gh, lazygit, nodejs, bun, go, uv, ruff, rustup, kubectl, helm, etc.
- **LSP servers** (`modules/lsp/`): nixd, gopls, typescript-language-server, terraform-ls, ty, yaml-language-server, marksman (`rust-analyzer` must be installed in the active rustup toolchain)
- **Devsisters-specific** (`base/devsisters/`): saml2aws, vault, kc2aws, wg-cli, scala, ruby, databricks-cli, pantry. `kc2aws` and `wg-cli` come from the private `keycloak2aws` and `devsisters-wg` flake inputs.
  - `kc2aws` comes from the private `keycloak2aws` flake input (`git+ssh://...`), pinned in `flake.lock`. To pull the latest `main`: `nix flake update keycloak2aws`, then `home-manager switch`. Commit the resulting `flake.lock` change.
