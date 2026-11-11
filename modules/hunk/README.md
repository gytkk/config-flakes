# Hunk

The common Home Manager profile enables `modules.hunk.enable` and installs
[files/config.toml](files/config.toml) at `~/.config/hunk/config.toml`.
The common profile installs the Hunk package directly from the root nixpkgs input;
its version follows the root `flake.lock`, not the local app update CI.

`theme = "auto"` selects `github-light-default` or `github-dark-default` from the
terminal background at startup, falling back to dark if the terminal does not
answer. Repository configuration or `--theme` can override this setting.

Apply with the usual Home Manager switch, then reopen Hunk. This module manages
the config as a read-only file; change preferences in the repository rather than
saving them from Hunk's theme picker.
