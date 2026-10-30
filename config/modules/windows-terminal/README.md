# Windows Terminal

On WSL, [default.nix](default.nix) merges [Herdr keybindings](files/herdr-keybindings.json) into the existing Windows Terminal settings. Missing settings are skipped. Profiles, themes, and unrelated keybindings are preserved.

Home Manager runs the entire update through its dry-run wrapper, including temporary file creation and JSON output. A live update writes to a unique temporary directory beside the settings, checks that jq succeeds, and replaces the settings by a rename. Before each changed update, `settings.json.home-manager.bak` receives the current settings. An unchanged update keeps the existing backup; invalid JSON or a failed replacement leaves the installed settings intact.

Run the isolated activation regression tests from the repository root with `uv run --no-project scripts/tests/test_config_activation.py`.
