# Launchd lifecycle backport

[The backport](backport.nix) replaces only `processAgentImpl` in the pinned Home Manager launchd module. It deliberately fails when that function's source changes.

Home Manager revision `b787c48e2ec57a9d098f5c2721882a3bc1e16f97` supplies the macOS 26 `bootout --wait` gate, harmless absent-domain handling, per-agent failure aggregation, domain migration, and retry of unchanged but unloaded agents. The local replacement preserves those behaviors and backs up the installed plist before replacement so rollback also works when the previous generation's plist is missing or differs from the installed file. A failed first bootstrap removes the newly installed plist so reapplication retries. It does not roll back the whole Home Manager generation.

Run the isolated behavioral fixture with `uv run --no-project -m unittest discover -s lib/launchd/tests -v`. It extracts the transformed activation body, injects fixture `launchctl` and `sw_vers` paths, and never contacts launchd. The fixture compares rollback with the pinned original behavior, tests reapplication and domain migration, and verifies that dry-run does not change installed files.

On a Home Manager input update, compare the new lifecycle code with both snippets and rerun the fixture. Remove this patch and its import when upstream provides the required behavior. Keep real macOS activation and login checks separate from the stubbed lifecycle tests.
