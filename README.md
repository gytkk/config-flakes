# config-flakes

This repository manages Home Manager and NixOS configuration alongside shared coding-agent rules and skills. Environment composition lives in `systems/`; reusable configuration components and the flake entry point remain at the repository root, and shared agent resources live in `agent-core/`.

| Path | Responsibility |
| --- | --- |
| [Configuration guide](#system-configuration) | Home Manager, NixOS, packages, secrets, and generated themes |
| [systems/](systems), [systems/inventory.nix](systems/inventory.nix) | Common Home Manager profiles, NixOS host composition, and environment definitions |
| [modules/](modules) | Application settings and Home Manager or NixOS integration |
| [lib/](lib) | Shared configuration builders and integration helpers |
| [packages/](packages/README.md) | Package definitions, update tools, and checks |
| [themes/](themes/README.md) | Shared theme definitions, templates, and generated exports |
| [secrets/](secrets) | Agenix-encrypted secrets and recipient rules |
| [agent-core/](agent-core/README.md) | Shared agent rules, adapters, skills, and renderer |
| [scripts/](scripts) | Documentation path checks and agent skill installation |
| [flake.nix](flake.nix), `flake.lock` | Direct configuration inputs and dependency pins; outputs are assembled in [outputs.nix](outputs.nix) |

Application and hardware projects are maintained in separate private repositories:

| Repository | Local checkout |
| --- | --- |
| [gytkk/menu](https://github.com/gytkk/menu) | `~/menu` |
| [gytkk/trading-agent](https://github.com/gytkk/trading-agent) | `~/trading-agent` |
| [gytkk/keyboards](https://github.com/gytkk/keyboards) | `~/keyboards` |
| [gytkk/keycult-trackball](https://github.com/gytkk/keycult-trackball) | `~/keycult-trackball` |

Each repository owns its source, history, checks, and project instructions. This repository retains the configuration flake, shared agent resources, and their supporting scripts, documentation, and CI. OpenClaw's independent workspace instructions, skills, and runbooks remain separate; see [OpenClaw task routing](modules/openclaw/README.md#task-routing-across-repositories).

The GitHub repository is `gytkk/config-flakes`. Runtime checkout paths default to `~/development/config-flakes` on every host. Keep old checkout paths available while installed generations reference them. Application settings belong in `modules/`; shared agent instructions and skills belong in `agent-core/`.

Run configuration commands from the repository root, using an environment or host name from [the inventory](systems/inventory.nix):

```bash
home-manager switch --flake .#<environment>
sudo nixos-rebuild switch --flake .#<host>
```

See [prerequisites](#prerequisites) and the configuration guide below for activation details, and the [agent-core guide](agent-core/README.md) for shared rules, skill installation, and validation. Worktrees can be used for editing and validation; switching from one requires configuring its runtime checkout path or applying from the main checkout.

## System configuration

`systems/` owns Home Manager and NixOS environment composition. Reusable modules, libraries, the package catalog, secrets, and the theme pipeline remain at the repository root. `flake.nix` declares inputs directly and delegates outputs to `outputs.nix`; `flake.lock` pins configuration dependencies. Run every command in this document from the repository root so updates and evaluation use that lockfile.

## Prerequisites

- Install Nix 2.26 or newer: <https://nixos.org/download>.
- Enable flakes:

```bash
sudo mkdir -p /etc/nix
echo "experimental-features = nix-command flakes" | sudo tee -a /etc/nix/nix.conf
```

- Several modules create out-of-store symlinks from the environment's `flakeDirectory`, defaulting to `~/development/config-flakes`. Set an override in `systems/inventory.nix` when needed.
- Standalone Home Manager commands evaluate without `--impure`. The checkout path still matters for modules that intentionally install out-of-store symlinks back to the repo.

## Checkout paths and migration

Runtime checkout paths default to `~/development/config-flakes`, including when evaluation runs in another worktree. Worktrees are allowed for editing and validation. Before switching from a worktree, deliberately configure its checkout path or apply the change from the main checkout so mutable links and runtime scripts point to the intended files.

After moving the checkout from `~/ws`, retain `~/ws` as a symlink to `~/development/config-flakes` while installed generations or runtime metadata still reference the old path. This restores links for generations using the current flat layout. Apply the appropriate Home Manager or NixOS switch command from the new checkout to install its configured paths. If OMP has replaced its broken configuration symlink with a regular file, preserve that file with a unique `-b` backup suffix when switching Home Manager, then review the backup for settings to retain.

The GitHub repository is `gytkk/config-flakes` (formerly `gytkk/ws`); renaming the remote does not require renaming the checkout. Keep `~/ws` for installed generations. Applications and hardware projects now live in their own private repositories, linked from the [repository navigation](#config-flakes). Root flake files, configuration CI, and supporting scripts remain here.

If installed generations still use `~/development/nix-flakes`, preserve that path too. Move each host's checkout before applying the new default, or set that host's `flakeDirectory` in `systems/inventory.nix` to its actual checkout path.

Environment composition now lives under `systems/`, while `modules`, `themes`, and `secrets` remain at the repository root. Installed generations from the previous layout still reference `~/ws/config/modules`, `~/ws/config/themes`, and `~/ws/config/secrets`, including runtime scripts and stored marketplace paths. This checkout ships no `config/` compatibility symlinks. Updating the checkout alone does not activate the new paths: each host must switch to the current Home Manager or NixOS configuration and verify its runtime links and stored marketplace paths. Before rolling back to a generation that uses the old `config/` layout, restore a matching old-layout checkout at the generation's expected path; otherwise its configuration links and runtime commands may fail.

## Secrets with agenix and 1Password

Encrypted secrets live in `secrets/*.age`, while `secrets/secrets.nix` defines which administrator, host, or workstation public keys may decrypt each file. Only public keys and encrypted files belong in Git.

- Home Manager uses each workstation's `~/.ssh/id_ed25519` as its agenix identity.
- NixOS hosts use their SSH host keys for unattended system secret activation.
- The current workstation key is the primary agenix administrator key. Keep its recovery copy in the 1Password item `pylv/agenix-admin-key`; never commit or symlink the private key into this repository.

Home Manager installs `agx`, which fixes the checkout's `secrets` directory, rules file, and `~/.ssh/id_ed25519` identity for every operation:

```bash
agx -e openai-api-key.age
agx -d cloudflare-tunnel-sepia-token.age
agx -r
```

`agx -d` writes plaintext to standard output. After adding or removing a recipient in `secrets/secrets.nix`, run `agx -r` and commit the rules and all changed `.age` files together. To recover the administrator identity, restore it from 1Password to `~/.ssh/id_ed25519`, set mode `0600`, and verify its public key matches `agenixAdmin` before editing or rekeying secrets.

## Architecture

This flake supports standalone Home Manager environments and NixOS hosts. macOS entries in `systems/inventory.nix` are Home Manager only; Linux NixOS entries compose system configuration plus a Home Manager user.

`systems/` groups environment composition separately from reusable `modules/`. It preserves the distinction between `systems/base/` (common Home Manager defaults and profile extensions) and `systems/hosts/` (host-specific NixOS imports and values), with `systems/inventory.nix` selecting the environments. Libraries, packages, themes, secrets, and shared agent resources keep their own root-level boundaries.

```text
flake.nix                         # Configuration inputs and output entry point
outputs.nix               # Home Manager, NixOS, package, app, and check outputs
systems/inventory.nix             # All Home Manager environments and NixOS hosts
systems/base/default.nix          # Common Home Manager imports and default enables
systems/base/<profile>/home.nix   # Profile-specific Home Manager extensions
agent-core/                       # Repository-root shared agent rules, adapters, skills, and renderer
modules/<name>/default.nix        # Reusable Home Manager or NixOS module
modules/nixos/                    # Common NixOS modules and shared secrets
systems/hosts/<name>/configuration.nix # Host-specific NixOS imports and values
packages/                        # Package catalog, shared helpers, updaters, and tests
packages/apps/                   # Non-nixpkgs app packages
lib/pkgs/default.nix              # Overlay and per-system package-set construction
lib/home-configurations.nix       # Home Manager configuration builder
lib/nixos-configurations.nix      # NixOS configuration builder
lib/builders.nix                  # Backward-compatible builder aggregation
```

Home Manager modules expose `modules.<name>.enable`; `systems/base/default.nix` owns common default enables, and profile files can override them. NixOS input modules that are host-specific, such as Disko, Copyparty, niri, and DankMaterialShell, are imported by the relevant `systems/hosts/<name>/configuration.nix`.

`lib/pkgs/` owns system package-set composition and overlay ordering; `packages/` owns the independent app catalog. LSP package selection and source pins remain in `lib/pkgs/lsp.nix`.

The common Home Manager profile installs GitUI in every environment. Run `gitui` inside a Git repository after applying the configuration.

The common profile also installs [Hunk](packages/README.md#updates-and-ci), an agent-aware diff TUI. After applying Home Manager, run `hunk diff` for local changes or `hunk show` for the latest commit. Its stable release pin follows the existing three-hour package update CI; installing those updates still requires pulling the commits and applying the configuration. The Git pager is unchanged. [Agent core](agent-core/README.md) separately installs the upstream `hunk-review` skill for Codex/Pi/OMP and Claude during Home Manager activation; it reads version-matched instructions through `hunk skill show hunk-review`.

The [Hunk module](modules/hunk/README.md) installs `theme = "auto"` in the user configuration, selecting a light or dark theme from the terminal background at startup.

On `pylv-onyx`, DankMaterialShell comes from the `dms` input, while the login screen uses nixpkgs' `services.displayManager.dms-greeter` module with niri in [the host configuration](systems/hosts/pylv-onyx/configuration.nix). The niri package uses the `niri` input's own pinned package set to keep its library dependencies compatible.

The [Onyx Home Manager profile](systems/base/pylv/onyx.nix) installs KiCad for PCB design and FreeCAD for mechanical CAD. Apply the configuration on Onyx with `sudo nixos-rebuild switch --flake .#pylv-onyx` from this checkout.

New Darwin environments leave [Claude session mining](modules/claude-session-mining/README.md) disabled. The two existing Mac entries select `systems/base/devsisters/session-mining.nix` in `systems/inventory.nix`; that fragment requires the external `gytkk-space` runner checkout. [Home Manager installation compatibility](systems/base/compat/README.md) is imported separately from common package and application defaults.

[Darwin agenix integration](modules/agenix-darwin/README.md) owns the stable login wrapper and activation retries. Both Home Manager builders use `lib/home-manager-modules.nix` for the explicit agenix mount interface and [launchd plist rollback compatibility](lib/launchd/README.md).

[Terraform version management](modules/terraform/README.md) uses tenv on Devsisters environments to select Terraform from project version files or `required_version` constraints and install missing versions on demand.

The Devsisters profile no longer installs Scala 2.12 or `saml2aws`; its login helper uses `kc2aws` alongside Vault and WireGuard. Projects that still need Scala should declare it in their project environment. Ruby remains installed from the existing stable nixpkgs pin.

VSCode is managed outside this configuration. The unused Home Manager module and its extension-catalog input have been removed; existing user-managed VSCode installations, settings, and extensions are untouched.

Themes use the local definitions and generated exports in [the theme pipeline](themes/README.md). The unused Rosé Pine and Catppuccin Ghostty source inputs have been removed; all existing theme variants and colors remain unchanged.

## Zsh config

The local [git-commit-suggestions Zsh plugin](modules/zsh/README.md#ai-commit-subjects) uses a Bun/TypeScript generator to request an AI commit subject when you type `gc -m`, using the staged diff, global/repository commit rules, and Minuet's OpenAI key. Accept the gray suggestion with the right arrow, review it, then press Enter to commit.

On macOS, login shells prioritize `~/.nix-profile/bin` after the system PATH setup, so commands such as `python3` use their Nix versions. See the [Zsh module README](modules/zsh/README.md#nix-executable-priority-on-macos) for application steps.

## Rust toolchain

Home Manager installs rustup, while rustup owns the toolchains and components under `~/.rustup`. Initialize the stable toolchain and the components used by editors and language-server integrations after the first Home Manager activation:

```bash
rustup toolchain install stable --profile default --component rust-analyzer rust-src
rustup default stable
```

Projects can use `rust-toolchain.toml` to select a different channel or component set.

## OpenClaw and shared agent configuration

OpenClaw on `pylv-onyx` is installed under `~/.openclaw` with the official rootless installer. OpenClaw owns its CLI, plugins, mutable configuration, and systemd user service. `modules/openclaw` provides the declarative NixOS and Home Manager integration.

Onyx uses the beta channel for the bundled [ChatGPT MCP relay](modules/openclaw/README.md#chatgpt-mcp-relay). Install or recover the user-owned beta release without onboarding:

```bash
curl -fsSL --proto '=https' --tlsv1.2 https://openclaw.ai/install-cli.sh | \
  bash -s -- --prefix ~/.openclaw --version beta --no-onboard
openclaw gateway install --force --wrapper ~/.openclaw/bin/openclaw
```

Use `openclaw update` for verified core and plugin updates. The mutable config owns the update channel and automatic updates. Nix does not pin or replace the OpenClaw executable.

`agent-core/` owns shared coding-agent rules, runtime adapters, local skills, and external skill source declarations. Nix installs generated instructions and extensions. [Agent skills activation](modules/agent-skills/README.md) installs or updates skills for all enabled agents on each Home Manager switch with pinned `skills@1.7.2`: Codex/Pi/OMP share the native pool and Claude uses global copies. OpenClaw is not part of this integration. Activation resolves the latest external source SHAs once for all targets and requires network access; Nix evaluation and builds remain offline for skill acquisition. See [Agent core](agent-core/README.md#skill-설치와-복구) for the one-time ownership handoff, recovery, and pending host migration gates. Run `nix run .#agent-core -- check` to validate canonical resources offline.

## Codex Plugin for Claude Code

Claude's [global instructions](modules/claude/README.md#claudemd) require messages with two or more paragraphs to include the English original and a complete Korean translation.

The official [openai/codex-plugin-cc](https://github.com/openai/codex-plugin-cc) plugin is installed, enabling Codex integration from within Claude Code. Superpowers is also installed for both Codex CLI and Claude Code from their official plugin marketplaces.

The `devsisters` base profile also installs the `devsisters/mumu` marketplace and `mumu@mumu` plugin. Its `claude` launcher adds the mumu settings through `--settings`; all profiles link user settings to the checkout so changes saved in Claude Code update the repository file. Remote marketplaces enable automatic plugin updates. See the [Claude module README](modules/claude/README.md#marketplaces) for update settings and profile-specific behavior.

The [LSP module](modules/lsp/default.nix) installs shared language servers. Claude and Codex use ty for Python, plus gopls, rust-analyzer, typescript-language-server, terraform-ls, and nixd. Codex connects through a read-only lspi MCP bridge; Rust uses the active rustup toolchain. See [Claude's plugin settings](modules/claude/README.md#installed-plugins) and [Codex's LSP integration](modules/codex/README.md#lsp-코드-탐색) for activation and workspace behavior.

[The LSP package selection](lib/pkgs/lsp.nix) selects ty, gopls, terraform-ls, nixd, yaml-language-server, and the VSCodium-derived HTML/CSS/JSON servers from the separate `nixpkgs-lsp` input declared in `flake.nix`. Update that input with `nix flake update nixpkgs-lsp` from the repository root. It also pins nixd and its matching libraries, plus VSCodium's extraction source, when their upstream releases are newer than that input. Review or remove those explicit source pins when updating the input so they do not override newer packages. Other packages keep the main nixpkgs pin. Apply Home Manager or NixOS configuration to install these server updates; rust-analyzer follows the active rustup toolchain separately.

### Available Commands

- `/codex:review`. Code review of current changes or branch
- `/codex:adversarial-review`. Challenge-based review questioning design decisions
- `/codex:rescue`. Delegate tasks to Codex (investigate bugs, fixes, etc.)
- `/codex:status`. Check running and recent Codex jobs
- `/codex:result`. View results of completed jobs
- `/codex:cancel`. Cancel active background jobs
- `/codex:setup`. Verify Codex installation and configuration

### Codex CLI Config

See the [Codex module README](modules/codex/README.md) for official bundle packaging, model delegation, reasoning effort, opening pages in the system default browser, configuration ownership, and activation behavior. The [Herdr integration](modules/codex/README.md#herdr-subagent-sidebar) runs Codex without the shared daemon inside Herdr so each pane retains its own session and subagent metadata.

Home Manager installs Ponytail only when it is missing and preserves its installed version and enabled state on later switches. Update it explicitly with `codex plugin add ponytail@ponytail`; changed plugin contents require hook approval again. See [Codex activation](modules/codex/README.md#적용) for hook trust and update behavior.

Codex and Claude Code follow shared instructions to display pages in the system default browser and use runtime- or project-provided tooling for automation, with headless mode as the default. See [Claude browser use](modules/claude/README.md#browser-use) for Playwright MCP cleanup and activation.

The Codex and Claude Herdr watchers share the [JSONL reader](modules/herdr/README.md#shared-jsonl-reader), which handles incremental reads, file replacement, and malformed records.

[Claude plugin activation](modules/claude/README.md#skills-and-plugins) checks marketplace revisions and caches successful plugin updates. Unchanged sources and installations skip update commands; network revision checks still run during each switch.

Codex and Claude share the `openai-decisions` skill through agent-core for `gpt-6-luna` Decisions API judgments. The [OpenAI API module](modules/openai-api/README.md) supplies `with-openai` to run API clients with the existing agenix-managed `OPENAI_API_KEY`.

- On NixOS hosts, the static Codex base config is installed to the overridable system layer at `/etc/codex/config.toml`.
- Repository-managed skill sources and selections are declared in `agent-core/manifest.toml`. [Skill activation](modules/agent-skills/README.md) checks source revisions and installation integrity, skipping unchanged installations into the Codex/Pi/OMP native pool at `~/.agents/skills` and Claude's skill directory.
- On standalone Home Manager environments, `home-manager switch` ensures `/etc/codex/config.toml` is a symlink to this repo's `modules/codex/files/config.toml`.
- Existing standalone `/etc/codex/skills` links require an ownership check and backup during migration; activation no longer manages them.
- Standalone activation only prompts for `sudo` when `/etc/codex` needs to be created or repaired, or when removing the legacy `/etc/codex/managed_config.toml` symlink.
- Using the repo path instead of the flake source store path avoids repeated sudo prompts after unrelated repo changes.
- If `/etc/codex/config.toml` or the legacy `/etc/codex/managed_config.toml` already exists as a regular file, activation stops instead of overwriting it.
- `~/.codex/config.toml` stays writable and is not rewritten by activation, preserving user-local state such as project trust, hook trust, notices, and TUI state.
- Maintain local skill content under `agent-core/skills/` and source selections in `agent-core/manifest.toml`. The installation command records exact external SHAs and backs up any installed skill before replacing it to match the manifest. Legacy skill renderer goldens remain frozen until all hosts migrate.
- Cloudflare operations use the shared [`cf` CLI](#cloudflare-cli).
- `home-manager switch` installs and enables `superpowers@openai-curated` from the official Codex marketplace and removes the legacy `~/.agents/skills/superpowers` link when it points to `~/.codex/superpowers/skills`. Installation failures stop activation and are recorded in `~/.codex/nix-setup.log`. See the [Codex module README](modules/codex/README.md#적용) for installation and restart instructions.

### Pi coding agent

Pi uses the official upstream Nix package definition, the generated `one-half-light` theme, and the `openai` provider's ChatGPT login. Home Manager synchronizes common settings into a device-local writable file, keeping installation IDs out of the checkout. The module does not install the legacy Codex usage extension. See the [Pi module README](modules/pi/README.md) for authentication, native MCP integration, background shell tasks, on-demand web and subagent tools, pinned extensions, and verification steps.

### OMP

The [OMP module](modules/omp/README.md) installs `omp` alongside Pi and manages `~/.omp/agent/config.yml` through an out-of-store symlink to the checkout. Its Pi-style status line includes OMP's built-in provider quota display. Existing local settings need a backup on the first switch; credentials, sessions, databases, and named profiles remain device-local. Pi settings and extensions are not migrated.

The [app package](packages/apps/omp/package.nix) uses OMP's official source-build flake at an immutable release revision, including upstream's locked toolchain and install checks. [App updates](packages/README.md#updates-and-ci) track stable OMP releases automatically; pull the resulting commits and apply Home Manager to install them. Restart `omp` after switching, then use `/login` if authentication is needed. `Alt+A` opens Agent Hub; `/agents` manages agent definitions and model overrides.

### Cloudflare CLI

The common Home Manager profile installs Cloudflare's [`cf` CLI](https://developers.cloudflare.com/cf/get-started/) from the pinned [app package](packages/README.md). It provides `cf` and `cloudflare` with a Nix-managed Node.js runtime. Wrangler and the Codex/Pi Cloudflare MCP registrations are replaced by this CLI; Claude activation removes the legacy user-scope `cloudflare` MCP registration. Restart the agents after applying the configuration.

`cf` is currently in beta. Existing Wrangler projects need the [documented migration](https://developers.cloudflare.com/cf/wrangler/) before using `cf dev`, `cf build`, or `cf deploy`.

Apply the configuration with `home-manager switch --flake .#<environment>` or `sudo nixos-rebuild switch --flake .#<host>`, then sign in once per machine. `cf` keeps separate credentials and does not reuse a Wrangler login:

```bash
cf --version
cf auth login
cf auth whoami
cf cli search "list DNS records"
```

For a remote machine, use `cf auth login --no-browser`. Automation can supply `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID`; the token takes precedence over saved profiles. The existing agenix-managed `cloudflare-access-api-token.age` remains available for token-based use. The Sepia `cloudflared` tunnel service and its separate tunnel credential remain required for server connectivity.

### Agent Session Record Hooks

- [Agent session record 모듈](modules/agent-session-record/README.md)은 `agent-session-record` CLI 하나를 설치한다. Claude의 `SessionEnd`, Codex의 `SessionStart`와 `Stop` hook은 `agent-session-record hook`을 사용한다. 전체 기록을 다시 처리할 때는 `agent-session-record replay <claude|codex>`를 실행한다. Home Manager가 실행 설정을 `~/.config/agent-session-record/config.json`에 생성한다.
- 직접 실행한 세션과 신원을 확인할 수 있는 subagent 세션에는 내용을 드러내지 않는 `run_id`를 부여한다. snapshot은 전송 전에 secret과 개인 정보를 가리고 로컬 private queue에 저장한다. 검사에 실패하거나 내용이 빈 snapshot도 보관하되 파생 지식에서는 제외한다.
- 전송에 성공하면 `/home/gytkk/agent-sessions/<scope>/<provider>/<YYYY>/<MM>/<DD>/<run_id>.*`에 archive를 남기고, manifest와 receipt를 로컬 ledger에 기록한다. 상태 디렉터리는 권한 `0700`, queue와 archive 파일은 권한 `0600`을 사용한다.
- 기본 scope는 `personal`이다. devsisters profile에서는 capture를 끄고, `pylv-denim`은 업로드 대상 `pylv-onyx`에 로컬 주소 `192.168.0.10`으로 접속한다.
- hook과 전송 실패는 각각 `warnings.log`와 `debug.log`에 기록하며 Claude와 Codex의 실행을 막지 않는다.
- 원격 업로드 단계는 전체 300초 제한 시간을 공유하며, timeout이면 로컬 전송 프로세스 그룹을 종료하고 snapshot을 queue에 남긴다. 살아 있는 작업자의 잠금은 경과 시간만으로 제거하지 않으며, 작업자는 자신이 생성한 잠금만 해제한다.

### Codex LSP MCP Implementation Plan

This section is a design plan only. The bridge and skills below are not implemented yet.

The current integration uses upstream lspi with Claude's language-server mappings as described in [the Codex module](modules/codex/README.md#lsp-코드-탐색). `codex-lsp-mcp` is a launcher for upstream lspi. The custom bridge, root detection, and skills below remain a proposal.

#### Goals

- Give Codex symbol-aware navigation and diagnostics through MCP instead of relying on plain text search alone.
- Reuse LSP binaries already installed by this flake when possible, while also supporting language servers that Claude Code treats as built-in.
- Auto-detect roughly ten languages from the current workspace without requiring per-repo manual MCP edits.
- Keep the first rollout read-only and deterministic so it is safe to enable by default in Codex.

#### Design Target

- Follow `lspi` for runtime design: explicit workspace roots, `doctor`, warmup, structured diagnostics, and server lifecycle management.
- Follow `symbols` for agent UX: a compact MCP tool surface plus bundled skills that teach Codex when to prefer semantic navigation over `rg`.
- Expose LSP functionality through a local `stdio` MCP server because Codex currently treats MCP as the primary extension path.

#### Planned Components

1. `codex-lsp-mcp` local server
   - Starts as a local `stdio` process from Codex config.
   - Owns language detection, LSP process supervision, tool dispatch, and diagnostic caching.
2. Profile registry
   - Data-driven definitions for each supported language instead of hardcoded branching.
   - Each profile includes file extensions, project markers, root markers, command candidates, default args, timeouts, and capability flags.
3. Workspace detector
   - Chooses the best profile from file extension plus nearby project files.
   - Resolves the workspace root before any LSP request so definitions and references stay scoped correctly.
4. LSP supervisor
   - Maintains one live session per `<workspace root, profile>`.
   - Supports lazy startup, optional warmup, restart-on-crash, and per-server logs for debugging.
5. Repo-local skills
   - Add a small skill set that nudges Codex toward semantic tools first for navigation, diagnostics, and safe refactors.

#### Initial Language Matrix

- `nix` via `nixd`, rooted by `flake.nix` or nearby `.nix` files
- `go` via `gopls`, rooted by `go.mod`
- `rust` via `rust-analyzer`, rooted by `Cargo.toml`
- `typescript` and `javascript` via `typescript-language-server`, rooted by `package.json`, `tsconfig.json`, or `jsconfig.json`
- `python` via `ty server` first, with room for a fallback profile later
- `terraform` via `terraform-ls serve`
- `yaml` via `yaml-language-server --stdio`
- `markdown` via `marksman server`
- one extra slot reserved for a future default such as `clangd` or `bash-language-server`

#### MCP Tool Shape

The first version should stay intentionally small:

- `hover_at`
- `find_definition_at`
- `find_references_at`
- `get_document_symbols`
- `search_workspace_symbols`
- `get_diagnostics`
- `doctor`

Resources should supplement the tools instead of expanding the tool count:

- `lsp://profiles`
- `lsp://profiles/<id>`
- `lsp://logs/<workspace>/<profile>`
- `lsp://diagnostics/<workspace>/<path>`

#### Detection and Routing Rules

- Prefer explicit project markers over extension-only guesses.
- Resolve the workspace root first, then select or start the matching server.
- Treat missing binaries as a diagnosable state, not a hard crash.
- Keep auto-detection separate from auto-installation. The first version only uses binaries already on `PATH`.
- Allow profile-specific adapters where raw LSP behavior is known to need extra shaping, especially for TypeScript-family servers.

#### Rollout Phases

1. Prototype the bridge
   - Build the MCP server with profile registry, detector, supervisor, and the seven read-only tools.
   - Add `doctor` output that explains which binaries, roots, and profiles were selected.
2. Integrate with `modules/codex`
   - Add the bridge as a default local MCP entry in `modules/codex/files/config.toml`.
   - Keep the bridge opt-in or read-only until the manual smoke tests are stable.
3. Add Codex skills
   - Ship a navigation skill and a diagnostics-first editing skill so Codex reaches for LSP tools before wide text scans.
4. Expand carefully
   - Add preview-first rename and other write-capable workflows only after the read-only path is stable.

#### Verification Plan

- Unit-test profile detection from extensions, markers, and missing-binary cases.
- Add fixture workspaces for at least `nix`, `go`, `rust`, `typescript`, and `python`.
- Verify `doctor` output for both healthy and degraded setups.
- Smoke-test MCP registration with `codex mcp list` and direct tool calls.
- Confirm that multiple files in the same workspace reuse one LSP session.
- Confirm that failures degrade to actionable diagnostics rather than hanging Codex.

#### Non-Goals for the First Iteration

- No raw 1:1 exposure of every LSP method.
- No automatic package downloads or language server installation.
- No write-capable refactors by default.
- No attempt to replace broad semantic search or indexing tools across the whole repository.

## macOS input switching

[gksdud](modules/gksdud/README.md) manages Right Command Korean/English switching on macOS. Automatic startup is controlled by the app's own login-item option. The module and package are excluded on Linux. See the module README for initial permissions and keyboard setup.

## Ghostty config

The shared `modules.commonTheme` default is `one-half-light` for Ghostty, Herdr, K9s, Neovim, Starship, and tmux. Zed and Pi select the same generated theme in their own settings. Its background and panel colors use cool whites and grays to avoid a yellow cast. See the [canonical themes documentation](themes/README.md#nix-integration) for export and installation behavior.

[Herdr](modules/herdr/README.md) uses generated custom colors with readable secondary text and darker gray backgrounds for the current space and agent in One Half Light, while preserving green running status icons under Ghostty's minimum contrast setting. [Claude Code](modules/claude/README.md) and [Git Delta](modules/git/README.md) use terminal palette themes configured for light backgrounds. Ghostty's generated One Half Light theme reduces the fading of dim text. [Codex](modules/codex/README.md) uses its built-in `one-half-light` syntax theme because the canonical pipeline has no Codex export.

Ghostty opens new windows maximized to the current monitor's available area. On macOS, it also preserves saved windows, tabs, and splits. See the [Ghostty module README](modules/ghostty/README.md) for applying the configuration and resizing restored windows.

## Neovim config

The [Neovim module](modules/vim/README.md) configures the nightly editor with LazyVim, Nix-installed language servers, shared themes, and personal Lua settings. The [keymap reference](modules/vim/README.md#notable-keymaps) covers the adopted LazyVim defaults, including `<Space>sR` to resume search and `<Space>sr` to search and replace. [Root detection and editing options](modules/vim/README.md#root-and-editing-defaults) use project-root searches and automatic writes during eligible commands, with all folds open in new buffers and restored sessions. Bare `vi` restores the working directory's session and opens its explorer at that directory. The statusline extends LazyVim's defaults with Minuet status, and manual formatting uses LazyVim's shared formatter. Blink displays ordinary completions in its menu, while Minuet automatically shows `gpt-6.1-terra` suggestions as dimmed inline text. `<Tab>` accepts a visible AI suggestion before falling back to ordinary completion. See the module's [rollout instructions](modules/vim/README.md#usage-and-rollout) when applying the configuration.

`H` and `L` use Vim's native screen-top and screen-bottom motions with a two-line `scrolloff` margin; `[b` and `]b` switch Neovim buffers.

## Zed config

- Zed is managed through the [Zed module](modules/zed/README.md).
- Scala syntax support remains installed, with its language server disabled.
- On macOS and non-WSL Linux, `home-manager switch` installs `~/.config/zed/settings.json` and `keymap.json` as out-of-store symlinks to the repo, and exposes the entire `themes/exports/zed` directory at `~/.config/zed/themes`.
- That means mutable settings can switch between generated theme names without requiring another switch just to materialize a newly referenced theme file.
- On WSL hosts, activation stages settings, keymaps, and generated themes before replacing files in the Windows Zed config directory. Each changed file keeps its previous contents in a `.bak` file; dry-run makes no changes.
- The checked-in defaults point both light and dark mode at the generated `One Half Light` theme.

## Herdr config

- Herdr is managed through `modules/herdr/default.nix` and enabled by default. See the [Herdr module README](modules/herdr/README.md) for terminal shortcuts and how to apply them, and [agent detection](modules/herdr/README.md#agent-detection) for the exclusion of Claude maintenance commands during Home Manager activation.
- Home Manager generates read-only `~/.config/herdr/config.toml` from `modules/herdr/files/config.toml` and the selected `themes/exports/herdr` theme. Edit these sources and apply Home Manager; Herdr's UI cannot persist settings to the generated file.
- The configured prefix is `Ctrl+Space`.
- `Ctrl+Space`, then `Shift+n`, opens a popup that selects a previously visited directory with `zoxide` and `fzf`, then creates and focuses a workspace there.
- `Ctrl+Shift+Tab` or `Ctrl+Tab` focuses the previous or next tab. In Ghostty on macOS, `Cmd+[` or `Cmd+]` focuses the previous or next workspace, `Cmd+Shift+[` or `Cmd+Shift+]` focuses the previous or next agent in sidebar order, `Cmd+t` creates a Herdr tab, and `Cmd+w` closes the current Herdr tab. `Cmd+1..9` focuses a workspace by index in Ghostty on macOS, and `Alt+1..9` does the same in Windows Terminal on WSL. `Ctrl+Space`, then `Shift+1..9` focuses the corresponding visible agent. The terminal configs forward direct shortcuts as distinct Kitty keyboard sequences so Herdr receives every modifier.
- `Alt+t` creates a Herdr tab and `Alt+w` closes it through Herdr's own bindings, including in Windows Terminal on WSL. These keys have no Windows Terminal overrides and retain their shell or application behavior outside Herdr.
- On WSL, the [Windows Terminal module](modules/windows-terminal/README.md) merges the corresponding `sendInput` actions into the existing `settings.json` during Home Manager activation. Existing profiles, themes, and unrelated keybindings are preserved; each changed update saves the previous settings in `settings.json.home-manager.bak`. Dry-run makes no changes.
- Herdr uses the native terminal cursor so pane applications such as Neovim can preserve mode-specific cursor shapes. On Windows or WSL, this may expose ConPTY cursor flicker that Herdr's default drawn cursor avoids.
- After applying Home Manager, reload a running server with `herdr server reload-config`.
- Agents entries show the agent name and reported model on their second row. Pi's [parent status integration](modules/pi/README.md#herdr-parent-status) and the [OMP integration](modules/omp/README.md#herdr-agents-sidebar) report working, idle, and blocked states. Claude, Pi, Codex, and OMP also display subagent names, models, and activity. Claude, Codex, and OMP show lifecycle status markers, including native subagents working in the background. See [subagents](modules/herdr/README.md#subagents) for model reporting, display limits, and applying the status separator patch, and the [Claude hook and watcher](modules/claude/README.md#herdr-subagent-sidebar), [Pi extension](modules/pi/README.md#herdr-subagent-sidebar), [Codex hook and watcher](modules/codex/README.md#herdr-subagent-sidebar), or [OMP extensions](modules/omp/README.md#herdr-agents-sidebar) for setup and verification.
- Home Manager registers the full [Herdr Annotate plugin](https://github.com/plannotator/herdr-annotate) from `packages/apps/herdr-annotate` in the Nix store. Its plugin source, annotation binary, and document review source and Cargo dependencies are pinned by hash. Nix builds the document reviewer from unpatched upstream sources, using its default colors. `Update App Versions` checks every three hours and [commits enabled app updates directly to main](packages/README.md#updates-and-ci), including Herdr, Annotate, and Auto Title, after patch checks, flake evaluation, and changed-package builds pass. Pull the changes and apply Home Manager to install them. Reapplying an older Home Manager generation registers that generation's plugin version while preserving annotation data.
- After `Ctrl+Space`, press `a` to annotate selected text, `Shift+y` to copy annotations, `Ctrl+a` to copy and archive, `m` to manage annotations, `o` to review documents, or `Shift+o` to review the agent's last reply. `Shift+a` remains the agent picker.
- Home Manager links `~/.config/plannotator-tui/config.toml` to `modules/herdr/files/plannotator-tui.toml`; reviews open in an overlay by default. On Linux, the global copy shortcuts require a working `wl-clipboard`, `xclip`, or `xsel` setup. Copying inside the annotation manager also supports OSC 52 through the viewing terminal.
- In Herdr 0.9.1, document overlays open in the server's active tab; `--deliver-to` selects the feedback recipient. When opening a review from an agent, resolve its pane with `herdr pane current --current`, focus it with `herdr agent focus <pane-id>`, then launch the review. Check the returned `tab_id` against the caller's tab.
- Home Manager also registers [Auto Title](https://github.com/kryptamine/herdr-auto-title) from `packages/apps/herdr-auto-title` in the Nix store. `Update App Versions` tracks stable releases and pins source and Go dependency hashes; Go is needed only during the Nix build. The plugin starts with the Herdr server, so linking or reloading config does not start it. After applying Home Manager, stop the server when its running tasks can be interrupted and reopen Herdr to start the plugin or load its updated binary. Its first run renames existing panes, including manually named panes; later manual renames are preserved.

### Herdr Core

The common profile installs [Herdr Core](packages/README.md#updates-and-ci) (`herdr-core`, also `hc`) on every device, including `pylv-onyx`, `pylv-sepia`, and both Macs. Core is the desktop app's terminal runtime, separate from Herdr TUI and its server. Home Manager also owns `~/.local/bin/herdr-core` and `~/.local/bin/hc`, so desktop launches and non-interactive SSH connections find the Nix-managed executable.

Core's stable release pin follows the existing three-hour package update CI. Pull the resulting commits and apply the configuration to install new versions; the desktop updater and `hc update` cannot replace Nix store files. To update the pin manually, run `bash packages/apps/herdr-core/update.sh` and `bash packages/scripts/sync-readme-versions.sh`.

For the first migration from the upstream installer, back up the existing `~/.local/bin/herdr-core` and `~/.local/bin/hc` entries before applying, or use Home Manager's `-b before-herdr-core-nix` backup option with a standalone switch. Do not delete `~/.local/share/herdr-core` or Core's state: running terminal holders may still use the old bundle. On this MacBook, use `home-manager switch -b before-herdr-core-nix --flake .#devsisters-macbook`.

After applying, restart the desktop app to refresh executable discovery. From a regular terminal outside Herdr, run `hc server restart` to hand off to the installed Core without ending running programs, then check `hc server status` for the version and `installed_by: "nix"`. The restart preserves the server's environment; do not use `server stop` for this migration. macOS keeps the upstream signed app bundle intact, but privacy permissions may need approval again after its location changes.

## tmux config

- tmux is managed through `modules/tmux/default.nix`.
- Home Manager installs tmux plus a `tm` session-manager wrapper and links `~/.config/tmux/tmux.conf` and `keybindings.conf` to `modules/tmux/files/` through out-of-store symlinks.
- Home Manager also links `~/.config/tmux/themes` to `themes/exports/tmux`, exposes the selected `modules.commonTheme` as `~/.config/tmux/theme.conf`, and sources that generated theme from the main tmux config.
- Running bare interactive `tm` outside tmux opens the fzf-backed session manager; `tmux` remains the original tmux binary. Use arrow keys to select a session, `Enter` to attach, type a new session name and press `Enter` to create it, `Ctrl+r` to rename, and `Ctrl+d` to delete.
- The checked-in tmux config uses `Ctrl+a` as the prefix, starts window and pane indexes at `1`, enables mouse support, and keeps the statusline at the top with generated canonical theme colors. Mouse-wheel scrolling moves one line per event in copy mode. Pressing the prefix accent-colors the session segment and shows key hints on the right; synchronized panes still show a `SYNC` indicator.
- Resize the active pane in five-cell steps with `Ctrl+a`, then `Ctrl+h`/`Ctrl+j`/`Ctrl+k`/`Ctrl+l`.
- Close the active pane immediately with `Ctrl+a`, then `x`; use uppercase `X` to close the entire window.
- tmux sets the outer terminal title to the current session name, so terminal tabs that use pane titles, including WezTerm, show the tmux session instead of the `tmux` process name.
- Pi windows use `pi` as their tmux window label instead of the underlying `node` process name and keep that label while tmux copy mode is active.
- Claude Code windows likewise use `claude` instead of the Nix wrapper name `.claude-wrapped`, including while tmux copy mode is active.
- tmux enables CSI-u extended keys so modified keys such as `Shift+Enter` survive through tmux into TUI apps like Codex. Restart tmux fully after changing this setting, because existing clients keep the old key mode.
- Press `Ctrl+a ?` for the key list, `Ctrl+a w` for the tree, and `Ctrl+a s` for the session tree.

## AeroSpace config

- AeroSpace config is managed through the [AeroSpace module](modules/aerospace/README.md) on Darwin hosts.
- `home-manager switch` installs `~/.config/aerospace/aerospace.toml` as an out-of-store symlink to `modules/aerospace/files/aerospace.toml`, so the repo file stays mutable.
- After linking the XDG config, activation removes matching legacy `~/.aerospace.toml` files or moves a differing file to `~/.aerospace.toml.pre-xdg-backup`. A conflicting backup stops migration and preserves both files. Dry-run skips migration.
- The checked-in config keeps `config-version = 2` plus AeroSpace's default `main` and `service` keybindings.
- AeroSpace starts automatically at macOS login. Newly detected windows go to their default workspaces: Brave Browser to `1`, Ghostty to `2`, Slack to `3`, Notion to `4`, Brave Browser Beta to `A`, and Discord to `S`.
- After editing the config, reload it with `Alt+Shift+;`, then `Esc`. Workspace rules apply when windows are detected; reloading does not move existing windows.
- Omitted scalar options still fall back to AeroSpace's built-in defaults, but keyboard bindings do not inherit from the built-in config and must be declared explicitly in the repo file.

## Standalone Home Manager

Standalone Home Manager outputs are still available for profile-only testing.

The [Zsh timing runner](modules/zsh/README.md#nix-command-timing) preserves terminal progress displays and prints a table of elapsed time between stage messages and the total after interactive Home Manager and NixOS rebuild commands, including `sudo nixos-rebuild`. Home Manager activation steps are shown individually; NixOS uses its own build and system activation stages.

```bash
home-manager switch --flake .#devsisters-macbook

home-manager switch --flake .#devsisters-macstudio

home-manager switch --flake .#pylv-denim

home-manager switch --flake .#pylv-onyx

home-manager switch --flake .#pylv-sepia
```

### Build without switching

```bash
home-manager build --flake .#devsisters-macbook
```

## NixOS

Available NixOS hosts:

- `pylv-onyx`
- [`pylv-sepia`](systems/hosts/pylv-sepia/README.md)

```bash
sudo nixos-rebuild switch --flake .#pylv-onyx

sudo nixos-rebuild switch --flake .#pylv-sepia
```

### Build without switching

```bash
nix build .#nixosConfigurations.pylv-onyx.config.system.build.toplevel

nix build .#nixosConfigurations.pylv-sepia.config.system.build.toplevel
```

### `pylv-onyx` OpenClaw Control UI access

- Local network: `http://pylv-onyx:18790` when LAN DNS/mDNS resolves, otherwise use the current Wi-Fi address `http://192.168.0.10:18790`
- The LAN path is an `nginx` reverse proxy on `wlo1` only; the real gateway stays loopback-only on `127.0.0.1:18789`
- Public URL: `https://claw.pylv.dev`, protected by the existing Cloudflare Access owner-email policy and OpenClaw's Gateway password and browser device pairing
- The Onyx Cloudflare Tunnel connects directly to `http://127.0.0.1:18789`. DNS and ingress are managed with `cf`; see [public Control UI operations](modules/openclaw/README.md#public-control-ui).
- The nginx listeners on `18790` and `18791` retain legacy bearer-token injection. It does not replace authentication while the Gateway uses password mode.
- `openclaw dashboard --no-open` prints the local dashboard URL. Use the public HTTPS URL for remote browser access.
- Declarative integration lives in [`modules/openclaw`](modules/openclaw). Nix supplies runtime dependencies, proxy authentication, the firewall rule, and the user service runtime paths. OpenClaw's Secret Store owns the Discord and Notion/Google automation credentials; see the [module README](modules/openclaw/README.md).
- Mutable state, configuration, and the executable live under `~/.openclaw`. The official CLI owns the systemd user service; Onyx follows the beta update channel for MCP Relay.
- Home Manager adds the user-owned CLI directory to the shell `PATH` and materializes a systemd environment drop-in for `libcap`. The OpenClaw installer owns the service executable and Node runtime. Nix does not set `OPENCLAW_NIX_MODE` or install an OpenClaw package.
- OpenClaw instructions, skills, plugins, and session recording are managed independently of the coding agents. Existing hosts must remove the old shared skill and plugin references before switching; see [removing the shared agent integration](modules/openclaw/README.md#removing-the-shared-agent-integration).

## Helpers

After changing module paths or configuration behavior, update the corresponding module README and its links in the root guidance. Check local documentation paths without evaluating Nix:

```bash
uv run --no-project scripts/check-paths.py
```

The checker covers local Markdown link targets and concrete repository paths in inline code and code blocks in the root entry-point documents. It skips placeholder and glob paths, installed paths, external URLs, and link anchors. Pass additional Markdown files as arguments to check them explicitly.

```bash
nix flake show

nix flake update

# Update a single input to its latest tracked ref (rewrites flake.lock only).
# Example: bump kc2aws (keycloak2aws input) to the latest commit on main,
# then apply it with a home-manager switch and commit the flake.lock change.
nix flake update keycloak2aws
```

Generic tooling that probes `nix build .` now hits a small compatibility package instead of failing on a missing default output. For actual host builds, keep using explicit flake attributes such as `.#homeConfigurations...` or `.#nixosConfigurations...`.

## References

- Nix Packages: <https://search.nixos.org/packages>
