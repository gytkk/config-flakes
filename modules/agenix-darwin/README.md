# agenix on Darwin

This internal module keeps decrypted secrets under the existing XDG state paths and runs agenix once at login. It preserves `RunAtLoad`, disables `KeepAlive`, and retains `~/.local/state/agenix-launchd-wrapper` as the launchd entry point. Home Manager still waits for `/nix/store` before invoking that path.

`lib/agenix-home.nix` exposes the upstream mount command as the read-only `age.mountingScript` option. The patch targets agenix revision `b027ee29d959fda4b60b57566d64c98a202e0feb` and fails evaluation if its source anchor changes. It does not copy the secret decryption implementation. `lib/home-manager-modules.nix` supplies this module to both Home Manager builders; remove the patch when the pinned upstream exposes an equivalent output.

Activation creates the launchd log directory and atomically updates the wrapper and selected mount command after `writeBoundary`, before `setupLaunchAgents`. Its store references keep the runner and mount command in the generation closure. Login and activation hold the same file lock while mounting. Successful unchanged activation skips mounting; login always mounts. A failed or interrupted mount clears the success marker, so the next activation or login retries. Activating a retained older generation selects its mount command again.

The launcher stores only command paths and synchronization state under `~/.local/state/agenix-launchd`. It delegates identities, decryption, secret permissions, and secret generations to agenix. Failures produce a diagnostic and do not trigger a launchd restart loop. Activation reports a warning and continues, matching the previous failure policy.

Run the isolated checks from the repository root:

```sh
uv run --no-project -m unittest discover -s modules/agenix-darwin/tests -v
nix eval --impure --offline --no-write-lock-file --file modules/agenix-darwin/tests.nix
```

These checks use temporary mount commands and evaluate synthetic configurations. They do not decrypt real secrets or change launchd services. After applying Home Manager on each Mac, verify secret access, unchanged reapplication, and login behavior. Stable plist contents alone do not establish that macOS background notifications have stopped.
