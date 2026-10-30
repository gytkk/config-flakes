# AGENTS.md

Read these instructions when changing `config/`, the root flake entry point, or configuration package CI. Paths in code examples are relative to the repository root; run commands there.

The repository-wide instructions in [../AGENTS.md](../AGENTS.md) also apply. This file owns configuration-specific rules and reference material.

## Operational Rules

### Critical Rules

- Worktrees are allowed for editing and validation. Runtime checkout paths still default to `~/ws`; switch from a worktree only after deliberately configuring its checkout path, or apply from the main checkout.
- Follow existing code patterns and module structure in this repository
- Use `nixfmt` to format all Nix files before committing
- Prefer narrow, fast checks. Run time-consuming Nix evaluation commands such as `nix eval` and `nix flake check --no-build` only when they are required to validate the requested change or the user explicitly requests them. Skip them for documentation-only changes, simple edits, and checks unrelated to the changed behavior.
- Skip Nix package build tests by default because they are expensive. Run `nix build`, `nix-build`, or `nix flake check` without `--no-build` only when the user explicitly requests a build or a concrete build-specific concern cannot be checked with formatting, static checks, targeted tests, or evaluation. A package change alone does not justify a build.
- When changing the canonical theme pipeline or generated theme exports, leave a local git commit in a sensible rollbackable unit before finishing the work
- Before applying or rolling back generations that use the former root configuration paths, read [checkout paths and migration](README.md#checkout-paths-and-migration).

### Documentation Guidelines

- Documentation for `config/` belongs in: `AGENTS.md`, `CLAUDE.md`, `README.md`, code comments (sparingly), commit messages
- When module paths or behavior change, update the module README and its root documentation links. Run `uv run --no-project scripts/check-paths.py` to check the current entry-point documents; pass other changed Markdown files explicitly.
- PR descriptions should focus on the change and its purpose, omitting routine validation sections (`검증`, `Validation`, or `Testing`) and checklists for human reviewers to manage. Include validation guidance only when a breaking change requires explicit attention. This affects PR descriptions only; required checks still run.

### Build/Test/Lint Commands

**Agent-safe commands** (run only as needed under the validation rules above):

```bash
nixfmt <file.nix>                  # Format Nix files
nix flake show                     # Show available flake outputs
nix flake check --no-build         # Validate complex or evaluation-sensitive changes
nix eval .#homeConfigurations.pylv-denim.config.home.packages --apply 'x: map (p: p.name) x' # Evaluate a specific output when required
```

**User-run commands** (ask the user to run these):

Standalone Home Manager commands do not require `--impure`, but several modules still install out-of-store symlinks to the checkout path.

```bash
home-manager switch --flake .#<environment>
nixos-rebuild switch --flake .#<host>
```

---

## Reference

### Nix Code Style

- **Formatting**: Enforced by `nixfmt` (2-space indent). Use `with pkgs;` for package lists.
- **Naming**: Variables/options use camelCase, files use lowercase, directories use lowercase/kebab, environments use kebab-case.
- **Module pattern**: Home Manager modules expose `options.modules.<name>.enable` and gate runtime config with `lib.mkIf cfg.enable`. `config/base/default.nix` owns common default enable values; profile files override with `lib.mkForce` or plain assignments when needed.
- **Imports**: Use relative paths, import directories by name (e.g., `../modules/claude`)
- **Conditionals**: `lib.mkIf`, `lib.mkMerge`, `lib.mkForce`
- **Host definition**: See `config/inventory.nix` for required fields (`kind`, `system`, `username`, `homeDirectory`, `profile`)
- **Secrets**: Use [agenix](https://github.com/ryantm/agenix). Run `agenix -e config/secrets/<name>.age` with the secret's filename; NixOS decrypts it to `/run/agenix/<secretName>`.

### Architecture

Nix flake configuration for standalone Home Manager and NixOS environments on macOS and Linux.

```text
flake.nix                         # Repository-root inputs and output entry point
config/outputs.nix               # Configuration outputs
config/inventory.nix                     # All Home Manager environments and NixOS hosts
config/base/default.nix                  # Common Home Manager configuration
config/base/<profile>/home.nix           # Profile-specific Home Manager extensions
agent-core/                       # Canonical agent rules, adapters, skills, and renderer
config/modules/<name>/default.nix        # Reusable Home Manager or NixOS module
config/modules/nixos/                    # Common NixOS modules and shared secrets
config/hosts/<name>/configuration.nix    # NixOS host configuration
config/packages/                        # Package catalog, shared helpers, updaters, and tests
config/packages/apps/                   # Non-nixpkgs app packages
config/lib/pkgs.nix                      # Overlay and per-system package-set construction
config/lib/home-configurations.nix       # Home Manager configuration builder
config/lib/nixos-configurations.nix      # NixOS configuration builder
config/lib/builders.nix                  # Backward-compatible builder aggregation
config/overlays/default.nix              # nixpkgs version overlays
config/secrets/secrets.nix               # Agenix recipient configuration
```

#### Environments

Defined in `config/inventory.nix` (single source of truth). `kind` field determines build type:

- **devsisters-macbook / devsisters-macstudio**: ARM64 macOS, devsisters base, home-only
- **pylv-denim**: x86_64 Linux/WSL, pylv base, home-only
- **pylv-sepia**: x86_64 Linux/NixOS server, pylv base (with Disko, agenix, copyparty)
- **pylv-onyx**: x86_64 Linux/NixOS, pylv base (with niri, DankMaterialShell, user-managed OpenClaw)

#### Base System

1. **`config/base/default.nix`**: Common config. core modules, standard dev packages, programs
2. **`config/base/devsisters/home.nix`**: saml2aws, vault, kc2aws, wg-cli, scala, ruby, databricks-cli, pantry, custom scripts
3. **`config/base/pylv/home.nix`**: Minimal (inherits base)
4. **`config/base/pylv/sepia.nix`**: Home Manager additions for `pylv-sepia`; system configuration lives in `config/hosts/pylv-sepia/configuration.nix`

### Module System

Each module in `config/modules/` manages a specific tool. **When modifying settings for any tool, look in the corresponding module directory first.** `agent-core/` owns canonical agent instructions and shared skills. Runtime modules consume generated outputs and must not reimplement render or selection logic.

Common NixOS modules live under `config/modules/nixos`; host-specific NixOS input modules and values live in `config/hosts/<name>/configuration.nix`. `config/modules/openclaw` keeps the NixOS and Home Manager integration for the user-managed OpenClaw install. It does not own the OpenClaw package, mutable state, or user service.

For theme-backed apps, prefer the canonical theme pipeline under `config/themes/` over app-local theme copies:

- `config/themes/core/` is the source of truth for reusable theme definitions
- `config/themes/templates/` defines app adapter/template structure
- `config/themes/exports/` contains generated app-ready artifacts
- when changing a module to use a theme, prefer wiring the module to `config/themes/exports/<app>/...` rather than creating or editing duplicated app-local theme files inside `config/modules/<app>/themes/`
- after changing theme templates, generators, overrides, or generated exports, leave the repo with a local commit that captures the change in a clear rollbackable unit

```text
config/modules/<name>/
├── default.nix    # Main module configuration (ALWAYS exists)
├── files/         # Static config files (JSON, TOML, etc.)
├── themes/        # Legacy or app-local themes (prefer `config/themes/exports/` for generated themes)
└── agents/        # AI agent definitions (for AI tools)
```

#### Module reference

This table lists selected modules, with paths relative to the repository root. See each module's `default.nix` for its installation rules.

| Module       | Purpose             | Key Files                                           | Mutable |
| ------------ | ------------------- | --------------------------------------------------- | ------- |
| `config/modules/nixos/` | NixOS common config | `config/modules/nixos/baseline.nix`, `config/modules/nixos/remote-access.nix`, `config/modules/nixos/user.nix` | NO |
| `config/modules/claude/` | Claude Code | `config/modules/claude/files/settings.json` | 부분적 |
| `config/modules/codex/` | OpenAI Codex CLI | `config/modules/codex/files/config.toml` | 부분적 |
| `config/modules/ghostty/` | Ghostty terminal | `config/modules/ghostty/files/config`, `config/themes/exports/ghostty` | NO |
| `config/modules/git/` | Git configuration | `config/modules/git/default.nix` | NO |
| `config/modules/k9s/` | Kubernetes manager | `config/modules/k9s/default.nix` | NO |
| `config/modules/lsp/` | LSP server packages | `config/modules/lsp/default.nix` | NO |
| `config/modules/terraform/` | Terraform versions | `config/modules/terraform/default.nix` (tenv) | NO |
| `config/modules/vim/` | Neovim | `config/modules/vim/files/lua/`, `config/modules/vim/files/lazy-lock.json`, `config/themes/exports/nvim` | 부분적 |
| `config/modules/vscode/` | VSCode (disabled) | `config/modules/vscode/default.nix`, `config/modules/vscode/one-half-light-theme/` | NO |
| `config/modules/zed/` | Zed editor | `config/modules/zed/files/settings.json`, `config/themes/exports/zed` | 부분적 |
| `config/modules/zsh/` | Zsh shell | `config/modules/zsh/default.nix`, `config/themes/exports/starship` | 부분적 |

> **Mutable**: `mkOutOfStoreSymlink`로 설치한 파일은 checkout을 직접 참조한다. 부분적 표시는 생성 파일이나 플랫폼별 복사와 함께 사용하는 경우다. 생성된 테마는 export를 직접 편집하지 않고 canonical 원본에서 수정한다.

### Editor-Specific Notes

#### Neovim (`config/modules/vim/`)

[Neovim README](modules/vim/README.md)에서 LazyVim 설정 구조와 적용 방식을 확인한다. LSP 서버를 추가할 때는 `config/modules/vim/files/lua/plugins/lsp.lua`의 서버 설정과 `config/modules/lsp/default.nix`의 바이너리를 함께 확인한다.

#### VSCode (`config/modules/vscode/`). DISABLED

현재 비활성화. VSCode 수정 요청 시 모듈 재활성화 여부를 먼저 확인할 것.

#### Zed (`config/modules/zed/`)

macOS와 WSL이 아닌 Linux에서는 설정과 keymap을 checkout으로 symlink하며, 테마는 `config/themes/exports/zed/`에서 가져온다. WSL에서는 activation이 설정, keymap, 생성 테마를 Windows 설정 디렉터리로 복사한다. Extensions는 `config/modules/zed/default.nix`의 `nixExtensions` 목록으로 관리한다.

#### Terraform

[Terraform README](modules/terraform/README.md)에서 `required_version`을 사용하는 tenv의 버전 선택과 자동 설치 방식을 확인한다. Terraform 전용 direnv hook은 필요하지 않으며, 기존 `.envrc`의 `use_terraform` 호출은 삭제한다.

### AI Coding Agent Notes

AI 코딩 에이전트 설정을 변경할 때 공통 지침, runtime adapter, shared skill은 `agent-core/`에서 수정한다. Runtime 전용 설정은 대응 모듈에서 수정하고, 이 저장소에서만 적용하는 작업 규칙은 루트 `AGENTS.md`에 둔다.

- `agent-core/rules/`는 공통 지침, `agent-core/adapters/`는 runtime별 지침, `agent-core/skills/`는 로컬 skill과 전환 중인 legacy source를 관리한다. `agent-core/manifest.toml`이 문서 조합, 로컬 및 외부 skill source와 설치 target을 정의한다.
- Skill은 필요한 capability를 runtime 중립적으로 표현한다. Runtime tool, SDK, metadata, plugin 설정은 대응 module이 소유하며, runtime module에 shared skill 복사본이나 별도 selection logic을 추가하지 않는다.
- Nix module은 `agent-core render`의 지침 문서를 설치한다. [Agent skills module](modules/agent-skills/README.md)은 Home Manager switch마다 활성 Codex, Claude, Pi의 skill 설치와 갱신을 `scripts/install-agent-skills.py`와 pinned Skills CLI로 실행한다. 최초 전환의 소유권 충돌, 복구 또는 host discovery를 다룰 때는 [Agent core README](../agent-core/README.md)를 읽는다. 모든 host의 코딩 에이전트 discovery 검증 전에는 해당 runtime의 legacy renderer와 vendored source를 정리하지 않는다.

- **Claude Code** (`config/modules/claude/`): `gytkk` marketplace의 원본은 `config/modules/claude/marketplace/`다. 공통 prompt는 agent-core output을 사용하고 shared skill은 CLI global copy로 설치한다. Plugin과 marketplace 목록은 [Claude README](modules/claude/README.md)에서 확인한다.
- **Codex CLI** (`config/modules/codex/`): 기본 설정은 `files/config.toml`에서 관리하고 최종 instruction은 agent-core output을 사용한다. Skill은 Pi와 `~/.agents/skills`를 공유한다.
- **Pi** (`config/modules/pi/`): Settings와 extension은 module에서 관리하고 최종 instruction은 agent-core output을 사용한다. Skill은 Codex와 native pool을 공유한다.
- **OpenClaw** (`config/modules/openclaw/`): Nix는 실행 경로와 service drop-in 등 host integration을 관리한다. OpenClaw의 지침, skill, plugin, 세션 기록은 코딩 에이전트와 독립적으로 관리한다. 기존 호스트에서 공통 연동을 해제할 때는 [OpenClaw README](modules/openclaw/README.md#removing-the-shared-agent-integration)를 읽는다. Mutable `openclaw.json`, workspace, auth/session state는 Nix가 소유하지 않는다.

### Package Management

- **Base packages** (`config/base/default.nix`): nixfmt, coreutils, findutils, docker, gcc, jq, fd, ripgrep, git, gh, lazygit, nodejs, bun, go, uv, ruff, rustup, kubectl, helm, etc.
- **LSP servers** (`config/modules/lsp/`): nixd, gopls, typescript-language-server, terraform-ls, ty, pyright, yaml-language-server, marksman (`rust-analyzer` must be installed in the active rustup toolchain)
- **Devsisters-specific** (`config/base/devsisters/`): saml2aws, vault, kc2aws, wg-cli, scala, ruby, databricks-cli, pantry. `kc2aws` and `wg-cli` come from the private `keycloak2aws` and `devsisters-wg` flake inputs.

The private `keycloak2aws` flake input is pinned in the root `flake.lock`. To update `kc2aws`, run `nix flake update keycloak2aws`, apply Home Manager, and commit the resulting lockfile change.
