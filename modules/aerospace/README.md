# AeroSpace

On Darwin, [default.nix](default.nix) links `~/.config/aerospace/aerospace.toml` to the checkout and runs legacy migration after Home Manager's `linkGeneration`. Dry-run skips migration and does not require the new link to exist.

Migration removes `~/.aerospace.toml` when it matches the managed config or the existing backup. Otherwise it moves the legacy file to `~/.aerospace.toml.pre-xdg-backup`. If that backup already contains different content, activation stops and preserves both files for manual reconciliation.

Run the isolated activation regression tests from the repository root with `uv run --no-project scripts/tests/test_config_activation.py`.
