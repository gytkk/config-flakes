# Home Manager compatibility

`home-manager.nix` keeps two repository-wide Home Manager compatibility settings outside the common base module.

`targets.darwin.copyApps.enable = false` keeps the current macOS App Management behavior. It was introduced when the Home Manager state version moved to 26.05 because `copyApps` became enabled from state version 25.11 and reset TCC App Management permission during activation. Reconsider it only after testing the current Home Manager behavior on a Darwin host that installs Nix-managed `.app` bundles.

The `home.activation.installPackages` override preserves the standalone Home Manager profile replacement flow added in commit `39e41ec`. Remove it only after validating the upstream activation flow on both manifest-based `nix profile` and legacy `nix-env` profile layouts used by this repository.
