# Zed

[default.nix](default.nix) installs settings, keymaps, generated themes, and extensions. On macOS and non-WSL Linux, settings and keymaps link to the checkout, and themes link to `themes/exports/zed`.

On WSL, Home Manager copies these files to Windows Zed. Activation stages and byte-compares all config files before replacing any destination. Each changed file is replaced by a rename, with its previous contents saved in a `.bak` file. Unchanged files keep their existing backup. Settings use Zed's JSONC syntax; the byte comparison checks copying, not the Zed configuration schema.

Extensions are copied to temporary directories before replacement. If replacement fails, activation restores the previous directory; if that restoration also fails, it retains the directory and reports its path. Dry-run makes no filesystem changes.

WSL destination and checkout paths are shell-escaped before activation. Spaces, apostrophes, dollar signs, and backticks in those paths remain literal filename characters.

Run the isolated activation regression tests from the repository root with `uv run --no-project scripts/tests/test_config_activation.py`.
