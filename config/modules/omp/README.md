# OMP

`modules.omp.enable` installs `pkgs.omp` and links `~/.omp/agent/config.yml` to `config/modules/omp/files/config.yml` in the checkout with `mkOutOfStoreSymlink`. The common Home Manager profile enables the module. `flakeDirectory` selects the checkout, normally `~/ws`.

Only the default profile's settings file is managed. Authentication, databases, sessions, caches, and named profiles remain device-local. Pi settings and extensions are not imported, and this module does not install shared instructions or skills. Do not put credentials or machine-specific paths in the shared YAML. Settings saved by OMP can change the checkout file; review those changes before committing.

## Status line

The layout follows the Pi footer using OMP's built-in segments:

- Left: working directory, Git branch/status, context usage, input/output tokens, and subagent count.
- Right: provider quota, model/thinking level, and session mode.
- The configured separator is `pipe`; the existing `claude` composer and themes are preserved. Composer shapes can arrange the segments differently.

`usage` reports the active provider/account's available quota windows, including Codex's five-hour and seven-day windows and time until reset. Percentages are **used**, not remaining: `7d 8%` means 92% remains. Unlike Pi's CodexBar extension, this uses OMP's account reports and does not need CodexBar. Missing quota data hides the segment. Use `/usage` or `omp usage` for the full report. A narrow terminal can omit segments.

## Agent UI

- `Alt+A` or `Ctrl+S`: Agent Hub, with live status, model, cost, activity, and transcripts.
- In the Hub: `Enter` focuses an agent, `t` toggles the hierarchy, `x` stops it, and `r` revives it. Type into a focused agent's editor to send instructions; `Esc` with an empty editor returns to the main session.
- `/agents`: agent-definition/settings UI, including enable/disable and model overrides.
- `/hotkeys`: inspect active bindings if a terminal intercepts these shortcuts.

## Apply

Run from the repository root. On the first standalone Home Manager switch, preserve an existing regular configuration file with a unique backup suffix:

```bash
home-manager switch -b before-omp-module --flake .#pylv-denim
```

Use the appropriate environment on other hosts. If that backup already exists, choose another suffix. Review each host's existing settings before switching; merge any desired differences into the shared YAML. Do not use `home.file.force` to discard local settings. NixOS users must preserve any existing conflicting file before their normal system switch.

Restart OMP after activation. `readlink -f ~/.omp/agent/config.yml` should resolve to the checkout file. OMP authentication remains independent of Pi; use `/login` if needed.

## Verification

From the repository root:

```bash
nixfmt config/base/default.nix config/modules/omp/default.nix
nix eval .#homeConfigurations.pylv-denim.config.modules.omp.enable
uv run --no-project scripts/check-paths.py config/modules/omp/README.md
omp config list --scope global --json
omp gallery --surface segment --segment usage --plain --width 160
```

The gallery uses fixture quota values, not the logged-in account's remaining quota. Before activation, configuration parsing and an interactive launch can be checked with a temporary `PI_CODING_AGENT_DIR` containing a copy of the shared YAML, without modifying the live profile.
