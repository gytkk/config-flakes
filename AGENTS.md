# Repository instructions

This is the `gytkk/config-flakes` repository: configuration and shared coding-agent resources. Application and hardware projects have their own repositories and checkout instructions. Preserve unrelated work.

- Before changing shared agent rules, runtime adapters, skills, source selections, or renderer behavior, read [agent-core/README.md](agent-core/README.md). Canonical shared sources remain in `agent-core/`; runtime settings belong in `modules/`.
- Declare configuration inputs directly in the root `flake.nix` and delegate configuration outputs to `outputs.nix`. Manage configuration dependency pins in the root `flake.lock`. Keep root `scripts/` for shared repository tools.
- Worktrees are allowed. Configuration runtime checkout paths default to `~/development/config-flakes`, independently of the evaluation checkout. Before applying Home Manager or NixOS configuration from a worktree, set the environment's `flakeDirectory` in `systems/inventory.nix` or apply from the main checkout.
- Keep environment composition in `systems/`: shared Home Manager defaults and profiles in `systems/base/`, NixOS host composition in `systems/hosts/`, and environment definitions in `systems/inventory.nix`. Reusable modules, libraries, packages, themes, and secrets remain at the repository root.
- Application and hardware source, checks, and project instructions belong in the [separate repositories](README.md#config-flakes). Read the target repository's instructions and run its checks there; this repository owns configuration and host integration, not those projects' implementation.
- Before changing OpenClaw resources or their paths, read [OpenClaw task routing](modules/openclaw/README.md#task-routing-across-repositories). Its independent workspace instructions, skills, and automation runbooks live outside this repository; shared coding-agent resources in `agent-core/` are not their replacement.
- Commit completed, validated task changes unless the user requests otherwise. Do not push unless explicitly requested.

For documentation changes, run `uv run --no-project scripts/check-paths.py` from the repository root; it checks `README.md` and `AGENTS.md`. Check additional changed Markdown files separately by passing their paths explicitly, since explicit arguments replace the default document list. Inline code paths must be repository-root-relative; the checker does not resolve project-relative code paths or validate anchors. Follow the affected subsystem's README for any additional checks. See [README.md](README.md) for repository navigation.

Run shell commands from the repository root, and interpret repository paths relative to it. Nix imports are relative to the importing file. The configuration-specific rules and reference material below apply to system configuration, the root flake, and configuration package CI.

## Operational Rules

### Critical Rules

- Follow existing code patterns and module structure in this repository
- Use `nixfmt` to format changed Nix files before committing.
- Prefer narrow, fast checks. Run time-consuming Nix evaluation commands such as `nix eval` and `nix flake check --no-build` only when they are required to validate the requested change or the user explicitly requests them. Skip them for documentation-only changes, simple edits, and checks unrelated to the changed behavior.
- Skip Nix package build tests by default because they are expensive. Run `nix build`, `nix-build`, or `nix flake check` without `--no-build` only when the user explicitly requests a build or a concrete build-specific concern cannot be checked with formatting, static checks, targeted tests, or evaluation. A package change alone does not justify a build.
- Before moving a checkout, retiring an old path, or applying or rolling back generations that reference `~/ws`, `~/development/nix-flakes`, or the former `config/` layout, read [checkout paths and migration](README.md#checkout-paths-and-migration). Preserve paths needed by installed generations; renaming the repository or moving the checkout is not a runtime cutover.

### Documentation Guidelines

- Repository-local agent guidance belongs in this root `AGENTS.md`. Keep configuration documentation in the root or affected subsystem's `README.md`, with code comments for local implementation constraints. Shared runtime instructions are generated from `agent-core/`, not maintained in a second root instruction file.
- When module paths or behavior change, update the module README and its links in `README.md` and repository-root guidance as applicable. Run `uv run --no-project scripts/check-paths.py` to check the current entry-point documents; pass other changed Markdown files explicitly.
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
sudo nixos-rebuild switch --flake .#<host>
```

---

## Reference

### Nix Code Style

- **Formatting**: Enforced by `nixfmt` (2-space indent). Use `with pkgs;` for package lists.
- **Naming**: Variables/options use camelCase, files use lowercase, directories use lowercase/kebab, environments use kebab-case.
- **Module pattern**: Home Manager modules expose `options.modules.<name>.enable` and gate runtime config with `lib.mkIf cfg.enable`. `systems/base/default.nix` owns common default enable values; profile files override with `lib.mkForce` or plain assignments when needed.
- **Imports**: Use paths relative to the importing Nix file; import module directories (e.g., `../../modules/claude` from `systems/base/default.nix`).
- **Conditionals**: `lib.mkIf`, `lib.mkMerge`, `lib.mkForce`
- **Host definition**: See `systems/inventory.nix` for required fields (`kind`, `system`, `username`, `homeDirectory`, `profile`)
- **Secrets**: Use [agenix](https://github.com/ryantm/agenix) through `agx -e <name>.age`; the Home Manager wrapper selects `secrets/`, its rules file, and the workstation identity. See [secret management](README.md#secrets-with-agenix-and-1password) for recovery and rekeying. NixOS decrypts secrets to `/run/agenix/<secretName>`.

### Architecture

Nix flake configuration for standalone Home Manager and NixOS environments on macOS and Linux. `systems/` groups environment composition separately from reusable `modules/`; `base/` and `hosts/` remain distinct concepts within that group.

```text
flake.nix                               # Configuration inputs and output entry point
outputs.nix                       # Configuration outputs
systems/inventory.nix             # All Home Manager environments and NixOS hosts
systems/base/default.nix          # Common Home Manager configuration
systems/base/<profile>/home.nix   # Profile-specific Home Manager extensions
agent-core/                             # Canonical agent rules, adapters, skills, and renderer
modules/<name>/default.nix        # Reusable Home Manager or NixOS module
modules/nixos/                    # Common NixOS modules and shared secrets
systems/hosts/<name>/configuration.nix # NixOS host configuration
packages/                        # Package catalog, shared helpers, updaters, and tests
packages/apps/                   # Non-nixpkgs app packages
lib/pkgs/default.nix              # Overlay and per-system package-set construction
lib/home-configurations.nix       # Home Manager configuration builder
lib/nixos-configurations.nix      # NixOS configuration builder
lib/builders.nix                  # Backward-compatible builder aggregation
lib/pkgs/lsp.nix                  # LSP package selection and source pins
secrets/secrets.nix               # Agenix recipient configuration
```

#### Environments

Defined in `systems/inventory.nix` (single source of truth). `kind` field determines build type:

- **devsisters-macbook / devsisters-macstudio**: ARM64 macOS, devsisters base, home-only
- **pylv-denim**: x86_64 Linux/WSL, pylv base, home-only
- **pylv-sepia**: x86_64 Linux/NixOS server, pylv base (with Disko, agenix, copyparty)
- **pylv-onyx**: x86_64 Linux/NixOS, pylv base (with niri, DankMaterialShell, user-managed OpenClaw)

#### Base System

1. **`systems/base/default.nix`**: Common config. core modules, standard dev packages, programs
2. **`systems/base/devsisters/home.nix`**: vault, kc2aws, wg-cli, ruby, databricks-cli, pantry, custom scripts
3. **`systems/base/pylv/home.nix`**: Minimal (inherits base)
4. **`systems/base/pylv/sepia.nix`**: Home Manager additions for `pylv-sepia`; system configuration lives in `systems/hosts/pylv-sepia/configuration.nix`

### Module System

Each module in `modules/` manages a specific tool. **When modifying settings for any tool, look in the corresponding module directory first.** `agent-core/` owns canonical agent instructions and shared skills. Runtime modules consume generated outputs and must not reimplement render or selection logic.

Common NixOS modules live under `modules/nixos`; host-specific NixOS input modules and values live in `systems/hosts/<name>/configuration.nix`. `modules/openclaw` keeps the NixOS and Home Manager integration for the user-managed OpenClaw install. It does not own the OpenClaw package, mutable state, or user service.

For theme-backed apps, prefer the canonical theme pipeline under `themes/` over app-local theme copies:

- `themes/core/` is the source of truth for reusable theme definitions
- `themes/templates/` defines app adapter/template structure
- `themes/exports/` contains generated app-ready artifacts
- when changing a module to use a theme, prefer wiring the module to `themes/exports/<app>/...` rather than creating or editing duplicated app-local theme files inside `modules/<app>/themes/`
- after changing theme templates, generators, overrides, or generated exports, leave the repo with a local commit that captures the change in a clear rollbackable unit

```text
modules/<name>/
├── default.nix    # Main module configuration (ALWAYS exists)
├── files/         # Static config files (JSON, TOML, etc.)
├── themes/        # Legacy or app-local themes (prefer `themes/exports/` for generated themes)
└── agents/        # AI agent definitions (for AI tools)
```

#### Module reference

This table lists selected modules, with paths relative to the repository root. See each module's `default.nix` for its installation rules.

| Module       | Purpose             | Key Files                                           | Mutable |
| ------------ | ------------------- | --------------------------------------------------- | ------- |
| `modules/nixos/` | NixOS common config | `modules/nixos/baseline.nix`, `modules/nixos/remote-access.nix`, `modules/nixos/user.nix` | NO |
| `modules/claude/` | Claude Code | `modules/claude/files/settings.json` | 부분적 |
| `modules/codex/` | OpenAI Codex CLI | `modules/codex/files/config.toml` | 부분적 |
| `modules/ghostty/` | Ghostty terminal | `modules/ghostty/files/config`, `themes/exports/ghostty` | NO |
| `modules/git/` | Git configuration | `modules/git/default.nix` | NO |
| `modules/k9s/` | Kubernetes manager | `modules/k9s/default.nix` | NO |
| `modules/lsp/` | LSP server packages | `modules/lsp/default.nix` | NO |
| `modules/terraform/` | Terraform versions | `modules/terraform/default.nix` (tenv) | NO |
| `modules/vim/` | Neovim | `modules/vim/files/lua/`, `modules/vim/files/lazy-lock.json`, `themes/exports/nvim` | 부분적 |
| `modules/zed/` | Zed editor | `modules/zed/files/settings.json`, `themes/exports/zed` | 부분적 |
| `modules/zsh/` | Zsh shell | `modules/zsh/default.nix`, `themes/exports/starship` | 부분적 |

> **Mutable**: `mkOutOfStoreSymlink`로 설치한 파일은 checkout을 직접 참조한다. 부분적 표시는 생성 파일이나 플랫폼별 복사와 함께 사용하는 경우다. 생성된 테마는 export를 직접 편집하지 않고 canonical 원본에서 수정한다.

### Editor-Specific Notes

#### Neovim (`modules/vim/`)

[Neovim README](modules/vim/README.md)에서 LazyVim 설정 구조와 적용 방식을 확인한다. LSP 서버를 추가할 때는 `modules/vim/files/lua/plugins/lsp.lua`의 서버 설정과 `modules/lsp/default.nix`의 바이너리를 함께 확인한다.

#### Zed (`modules/zed/`)

macOS와 WSL이 아닌 Linux에서는 설정과 keymap을 checkout으로 symlink하며, 테마는 `themes/exports/zed/`에서 가져온다. WSL에서는 activation이 설정, keymap, 생성 테마를 Windows 설정 디렉터리로 복사한다. Extensions는 `modules/zed/default.nix`의 `nixExtensions` 목록으로 관리한다.

#### Terraform

[Terraform README](modules/terraform/README.md)에서 `required_version`을 사용하는 tenv의 버전 선택과 자동 설치 방식을 확인한다. Terraform 전용 direnv hook은 필요하지 않으며, 기존 `.envrc`의 `use_terraform` 호출은 삭제한다.

### AI Coding Agent Notes

AI 코딩 에이전트 설정을 변경할 때 공통 지침, runtime adapter, shared skill은 `agent-core/`에서 수정한다. Runtime 전용 설정은 대응 모듈에서 수정하고, 이 저장소에서만 적용하는 작업 규칙은 루트 `AGENTS.md`에 둔다.

- `agent-core/rules/`는 공통 지침, `agent-core/adapters/`는 runtime별 지침, `agent-core/skills/`는 로컬 skill과 전환 중인 legacy source를 관리한다. `agent-core/manifest.toml`이 문서 조합, 로컬 및 외부 skill source와 설치 target을 정의한다.
- Skill은 필요한 capability를 runtime 중립적으로 표현한다. Runtime tool, SDK, metadata, plugin 설정은 대응 module이 소유하며, runtime module에 shared skill 복사본이나 별도 selection logic을 추가하지 않는다.
- Nix module은 Claude/Codex/Pi의 `agent-core render` 지침 문서를 설치한다. [Agent skills module](modules/agent-skills/README.md)은 Home Manager switch마다 활성 Codex, Claude, Pi, OMP의 skill 설치와 갱신을 `scripts/install-agent-skills.py`와 pinned Skills CLI로 실행한다. OMP는 shared skill 소비자이며 별도 renderer output을 만들지 않는다. 최초 전환의 소유권 충돌, 복구 또는 host discovery를 다룰 때는 [Agent core README](agent-core/README.md)를 읽는다. 모든 host의 코딩 에이전트 discovery 검증 전에는 해당 runtime의 legacy renderer와 vendored source를 정리하지 않는다.

- **Claude Code** (`modules/claude/`): `gytkk` marketplace의 원본은 `modules/claude/marketplace/`다. 공통 prompt는 agent-core output을 사용하고 shared skill은 CLI global copy로 설치한다. Plugin과 marketplace 목록은 [Claude README](modules/claude/README.md)에서 확인한다.
- **Codex CLI** (`modules/codex/`): 기본 설정은 `files/config.toml`에서 관리하고 최종 instruction은 agent-core output을 사용한다. Skill은 Pi와 `~/.agents/skills`를 공유한다.
- **Pi** (`modules/pi/`): Settings와 extension은 module에서 관리하고 최종 instruction은 agent-core output을 사용한다. Skill은 Codex와 native pool을 공유한다.
- **OMP** (`modules/omp/`): 기본 profile의 settings와 personality는 module의 checkout-backed 파일로 관리한다. Shared skill은 Codex/Pi와 `~/.agents/skills`를 공유하지만 agent-core의 생성 지침은 설치하지 않는다. 인증, 세션, named profile과 MCP 설정은 device-local이다. 변경 전 [OMP README](modules/omp/README.md)에서 소유권과 적용 절차를 확인한다.
- **OpenClaw** (`modules/openclaw/`): Nix는 실행 경로와 service drop-in 등 host integration을 관리한다. OpenClaw의 지침, skill, plugin, 세션 기록은 코딩 에이전트와 독립적으로 관리한다. 기존 호스트에서 공통 연동을 해제할 때는 [OpenClaw README](modules/openclaw/README.md#removing-the-shared-agent-integration)를 읽는다. Mutable `openclaw.json`, workspace, auth/session state는 Nix가 소유하지 않는다.

### Package Management

Before adding or updating app packages or changing their CI, read [the package guide](packages/README.md). `packages/default.nix` is the shared catalog consumed by the root outputs, configuration overlay, and nested package flake.

- Root `flake.nix` and `flake.lock` own configuration inputs, including private repositories. `packages/flake.nix` and `packages/flake.lock` independently own package CI inputs; keep updates scoped to the intended flake.
- When package evaluation is required, run `nix flake check ./packages --no-build` from the repository root. Use the affected package's documented checks rather than evaluating the configuration flake for package-only changes.
- `.github/workflows/apps-ci.yml` and `.github/workflows/apps-update.yml` validate the nested package flake, not Home Manager or NixOS integration. Changes to root inputs, outputs, or package-set wiring need their own targeted configuration validation; a passing package CI is not that evidence.

- **Base packages** (`systems/base/default.nix`): nixfmt, coreutils, findutils, docker, gcc, jq, fd, ripgrep, git, gh, lazygit, nodejs, bun, go, uv, ruff, rustup, kubectl, helm, etc.
- **LSP servers** (`modules/lsp/`): nixd, gopls, typescript-language-server, terraform-ls, ty, pyright, yaml-language-server, marksman (`rust-analyzer` must be installed in the active rustup toolchain)
- **Devsisters-specific** (`systems/base/devsisters/`): vault, kc2aws, wg-cli, ruby, databricks-cli, pantry. `kc2aws` and `wg-cli` come from the private `keycloak2aws` and `devsisters-wg` flake inputs.

The private `keycloak2aws` flake input is declared in `flake.nix` and pinned in the root `flake.lock`. To update `kc2aws`, run `nix flake update keycloak2aws` from the repository root, apply Home Manager, and commit the resulting lockfile change.
