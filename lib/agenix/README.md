# agenix Home Manager integration

`lib/agenix/default.nix` combines the patched upstream agenix module with the Darwin integration. `lib/home-manager-modules.nix` supplies both through one import to the standalone and NixOS Home Manager builders. Profiles and consumer modules retain responsibility for identity paths and secret declarations; they do not import the Darwin integration separately.

On Darwin, `lib/agenix/darwin.nix` keeps decrypted secrets under the existing XDG state paths and runs agenix once at login. It preserves `RunAtLoad`, disables `KeepAlive`, and retains `~/.local/state/agenix-launchd-wrapper` as the launchd entry point. Home Manager still waits for `/nix/store` before invoking that path.

`lib/agenix/patch.nix` exposes the upstream mount command as the read-only `age.mountingScript` option. The patch applies to the agenix Home Manager module pinned in [flake.lock](../../flake.lock) and fails evaluation if its `options.age` source anchor changes. It does not copy the secret decryption implementation. Remove the patch when the pinned upstream exposes an equivalent output.

Activation creates the launchd log directory and atomically updates the wrapper and selected mount command after `writeBoundary`, before `setupLaunchAgents`. Its store references keep the runner and mount command in the generation closure. Login and activation hold the same file lock while mounting. Successful unchanged activation skips mounting; login always mounts. A failed or interrupted mount clears the success marker, so the next activation or login retries. Activating a retained older generation selects its mount command again.

The launcher stores only command paths and synchronization state under `~/.local/state/agenix-launchd`. It delegates identities, decryption, secret permissions, and secret generations to agenix. Failures produce a diagnostic and do not trigger a launchd restart loop. Activation reports a warning and continues, matching the previous failure policy.

Run the isolated Python checks and synthetic Nix configuration evaluation from the repository root:

```sh
uv run --no-project -m unittest discover -s lib/agenix/tests -v
nix eval --impure --raw --file lib/agenix/tests.nix
```

The Python checks use temporary mount commands; they do not decrypt real secrets or change launchd services. The Nix fixture resolves this checkout's root flake, imports the shared Home Manager module list, and checks Darwin, empty-secret, and Linux configuration contracts without building or activating a configuration. `--impure` permits `builtins.getFlake` to resolve the local checkout path.

After applying Home Manager on each Mac, verify secret access, unchanged reapplication, and login behavior. Stable plist contents alone do not establish that macOS background notifications have stopped.
