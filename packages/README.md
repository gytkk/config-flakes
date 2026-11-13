# App packages

Self-contained non-nixpkgs app packages used by `gytkk/config-flakes`. The nested flake keeps package CI independent from the root configuration flake's private inputs.

The nested flake exports `aarch64-darwin`, `x86_64-linux`, and `aarch64-linux`. Intel Mac (`x86_64-darwin`) compatibility is not maintained.

`cf`, `databricks-cli`, `hunk`, and Datadog's `datadog-pup` are installed directly from the parent configuration's root nixpkgs input. They are not exported by this catalog or either flake's local app package outputs, and the app update/build CI does not manage them. Their versions follow the root `flake.lock`; the `datadog-pup` package provides the `pup` command (nixpkgs' `pup` attribute is a different HTML-processing tool).

The [trading-agent repository](https://github.com/gytkk/trading-agent) owns its Rust and web package build and exports `nixosModules.default` for its authenticated read-only admin containers. Host configurations consume a verified, pinned product revision and supply host settings and runtime file paths. This catalog does not duplicate the product package or service definitions. Trading supervisor deployment remains separate from the read-only containers.

## Layout

```
.
├── apps/
│   ├── claude-code/
│   ├── codex/
│   ├── codexbar/
│   ├── gksdud/
│   ├── herdr/
│   ├── herdr-core/
│   ├── notion-cli/
│   ├── pantry/
│   └── pi/
├── lib/
│   └── mk-tarball-cli.nix
├── default.nix
├── flake.nix
├── flake.lock
├── scripts/
│   ├── check-nix-patches.sh
│   ├── detect-changed-apps.sh
│   ├── sync-readme-versions.sh
│   └── update-all.sh
├── tests/
│   ├── check-nix-patches.sh
│   ├── codex-check-package.sh
│   ├── detect-changed-apps.sh
│   ├── herdr-core-update.sh
│   ├── notion-cli-update.sh
│   ├── pi-package.sh
│   ├── pi-file-clicks.test.ts
│   ├── pi-file-links.test.ts
│   ├── pi-update.sh
│   ├── sync-readme-versions.sh
│   └── update-all.sh
├── settings.json
└── README.md
```

## App versions

| App | Version |
|-----|---------|
| claude-code | 2.1.323 |
| codex | 0.169.0 |
| codexbar | 0.84.0 |
| gksdud | 2.1.0 |
| herdr | 0.9.5 |
| herdr-core | 0.1.21 |
| notion-cli | 0.23.30 |
| pantry | main-69de978b5555 |
| pi | 1.5.2 |

## Build entrypoints

- `nix build ./packages#packages.<system>.ntn`
- `nix build ./packages#packages.<system>.default` (the alphabetically first app, currently `claude-code`)
- `nix run ./packages#apps.<system>.ntn`

`gksdud` is exposed only on Darwin. Its signed universal app bundle supports Apple Silicon and Intel Macs running macOS 13 or later. Home Manager installs it through the [gksdud module](../modules/gksdud/README.md). Automatic startup is controlled by the app's own login-item option.

## Adding new apps

To add a new app package:

1. Create `packages/apps/<app-name>/package.nix`.
2. Use `callPackage` arguments available from nixpkgs (`stdenvNoCC`, `fetchzip`, etc.).
3. Ensure the package path creates a `meta.mainProgram` if the package should be run via `nix run`.
4. Add an executable `packages/apps/<app-name>/update.sh` if the package should support aggregate updates; non-executable scripts are skipped.
5. Add the package to `packages/default.nix` so the nested flake and parent package outputs expose it. The configuration overlay consumes this catalog.

To disable aggregate updates for an app, add its name to the `update.deny` list in `settings.json`.

The package catalog in `default.nix` is the single source of truth for exported apps and aliases. Register each package there once; the parent package outputs, nested flake, and configuration overlay in `lib/pkgs/default.nix` expose the full catalog. App-specific settings still belong in `modules/<app>/`, and enabling a package in a profile remains a separate choice.

`ntn` is an alias of `notion-cli` in the catalog. Both attributes provide the same derivation and the `ntn` executable. Aliases need no package directory or updater and do not add duplicate rows to the version table.

## Reusing the tarball CLI builder

The catalog supplies `mkTarballCli` to package functions that request it. Use it for a release tarball containing one executable with optional documentation files. [Notion CLI](apps/notion-cli/package.nix) shows a nested archive directory and additional installed files.

| Argument | Meaning |
| --- | --- |
| `pname`, `version` | Package identity, kept in the app's `package.nix`. |
| `platforms` | Attribute set keyed by Nix system. Each entry contains `hash` and any fields needed to construct the URL. |
| `url` | Function from the selected platform entry to the archive URL. |
| `meta` | Normal Nix metadata, including the required `mainProgram`. The executable in the archive must have that name. |
| `sourceRoot` | Optional function from the platform entry to the unpacked directory. Defaults to `_: "."`. |
| `extraInstall` | Optional shell commands after installing the executable and before `postInstall`, for example README and license installation. |

The helper selects the host platform, fetches the pinned archive, disables configure/build phases, installs the executable, runs install hooks, and derives `meta.platforms`. URL conventions, hashes, licenses, and archive-specific facts remain in the app file. Packages with multiple archives, binary patching, wrappers, or source builds continue to use their own `callPackage` expressions.

## Updates and CI

- Run `packages/scripts/update-all.sh` from the parent repository to update every enabled package manually.
- Both workflows use GitHub-hosted `ubuntu-latest` runners, install Nix 2.29.1 with an explicit `cachix/install-nix-action` installer URL, and install ripgrep for the updater and test scripts. Nix 2.35.6's lazy source copying breaks updater reads of `nix flake metadata` paths; the initial 2.34.9 compatibility pin passed evaluation but its daemon was OOM-killed while building OMP dependencies on a fresh runner. Nix 2.29.1 passed the [complete Linux package CI](https://github.com/gytkk/config-flakes/actions/runs/43132568111), including the former OMP package's build and install checks. Revalidate source access and package builds before upgrading Nix. Package builds no longer run on the production `pylv-sepia` host.
- Before building, both workflows remove the hosted runner's unused Android and .NET SDK directories (`/usr/local/lib/android` and `/usr/share/dotnet`). This was required by the former OMP package's native Rust build, which exhausted the runner disk even after the daemon OOM was avoided. Failed builds print memory, disk, Nix daemon, and kernel diagnostics; build and install-check failures remain fatal.
- `Update App Versions` checks all enabled packages, including `herdr`, every three hours in one serialized job. It also runs on pushes to `main` affecting `packages/**` or its own workflow, and through manual dispatch. After updater patch checks, flake evaluation, and changed-package builds succeed, it synchronizes the README version table and commits the updates directly to `main` without a PR or approval step.
- README synchronization accepts exactly one single-line literal `version = "...";` assignment per package, with a nonempty version containing only letters, digits, `.`, `+`, `_`, and `-` (starting with a letter or digit). Release versions and `main-<commit>` pins are supported; expressions, interpolation, missing versions, and duplicate assignments fail before publication. The version section and replacement README use unique same-directory staging files that are cleaned up on failure; the original README and any preexisting `README.md.tmp` remain untouched on parse or render failure. Run `bash packages/tests/sync-readme-versions.sh` for fixture-based synchronization and failure-preservation checks.
- Any updater, evaluation, or build failure stops the job before publication, so no updates from that run reach `main`. A patch preflight failure is reported before that updater changes package metadata. If `main` advances during validation, the push is rejected and a later run validates updates against the new base.
- The update job uses `GITHUB_TOKEN` with job-scoped `contents: write`. CI currently builds `x86_64-linux`; other platforms are not validated by this workflow.
- `herdr` tracks stable releases and builds from Rust and Zig sources with patches for [Claude administrative command detection](../modules/herdr/README.md#agent-detection) and sidebar status separators. On Linux, the static Ghostty library uses the compiler runtime supplied by the Rust toolchain instead of bundling Zig 0.16's runtime, whose invalid unwind relocations fail to link with GNU ld. Package checks cover plugin APIs and process identification. Cargo dependencies come from the source's `Cargo.lock`; Zig dependencies use its vendored Nix manifest. The hash-pinned source is fetched during evaluation so `nix flake check --no-build` can read both files without building a source derivation first. The executable replacement test uses the build's temporary directory so it can run inside the Linux Nix sandbox. Older nixpkgs inputs use the pinned Zig 0.16 build tool in `apps/herdr/zig.nix`. Its updater pins the source hash and checks every declared patch and the Zig requirement before changing the package.
- `herdr-core` tracks the stable [Core release manifest](https://releases.herdr.dev/core/latest.json). Its updater verifies all four macOS/Linux archives against the manifest's SHA-256 checksums before atomically replacing the version and hashes. macOS keeps the signed `Herdr Core.app` bundle intact; Linux uses the upstream static executable. Both `herdr-core` and `hc` are installed in every environment, independently of Herdr TUI. The three-hour update job advances the pin; pull its commits and apply Home Manager or NixOS to install updates. `hc update` and the desktop updater defer to Nix rather than replacing store files. Run `bash packages/tests/herdr-core-update.sh` for updater failure-preservation checks.
- Herdr declares its patch order in `patches.nix`, shared by the Nix build and updater preflight. Preflight applies the complete sequence to a writable temporary source copy, retaining Nix's default fuzz tolerance and rejecting reversed patches. Run `bash packages/tests/check-nix-patches.sh` to check this behavior, and `bash packages/tests/update-all.sh` to check updater selection and failure propagation with local fixtures.
- `pi` follows the source-build approach in upstream's [Run with Nix](https://github.com/earendil-works/pi#run-with-nix): it pins a stable release source archive and calls the official flake's `nix/package.nix` without replacing its build or install-check phases. Like upstream's overlay, it uses the consuming catalog's nixpkgs rather than the upstream flake's nixpkgs lock. Upstream owns the workspace build, install lock, model catalog pin, platform dependencies, and install checks; this repository does not maintain a separate npm lockfile. The local `patches.nix` and `herdr-file-opener.ts` route [Pi file-link Ctrl-clicks](../modules/pi/README.md#file-links-in-herdr) to Neovim in a Herdr split. The package adds the patch and helper only to upstream's `pi-workspace-packages` derivation through its `stdenv` argument. Metadata and lockfiles remain readable from the original source without import-from-derivation; the final install-lock derivation is not patched. Home Manager installs the catalog package, so it does not need a second `nix profile add` installation. The updater validates the release version, required packaging files, and patch applicability before changing the source pin. Run `bash packages/tests/pi-update.sh` for its failure and no-op checks and `bun test packages/tests/pi-file-links.test.ts packages/tests/pi-file-clicks.test.ts` for the file opener and click dispatch.
  Set `PI_TUI_SOURCE` to the patched upstream TUI directory (for example, `/path/to/patched-pi/packages/tui/src`) before running the click-dispatch suite; without it, that suite is skipped.
- Pi's `packages/apps/pi/message-renderer-wrappers.patch` adds the TUI-only `registerMessageRendererWrapper` extension hook used by [rounded extension messages](../modules/pi/README.md#rounded-tool-and-extension-message-frames). It decorates both custom renderers and default Markdown without changing model context or saved message data. With an unpatched source checkout matching the installed Pi version, run `PI_SOURCE=/path/to/pi-source bun packages/tests/pi-message-wrappers.ts` from the repository root. This applies the patch in a temporary tree, transpiles only affected runtime modules into a temporary copy of the installed package, and runs renderer/loader integration tests. It neither installs dependencies nor builds or replaces the live Pi package. `bash packages/tests/pi-update.sh` also verifies that incompatible wrapper patches block package updates.
- The Pi schema-package workaround in `package.nix` is gated to version `1.5.0` and is inactive for the current pin. For that release only, it adds `schema.tgz` to workspace packing and the `importNpmLock` package-source overrides. Run `bash packages/tests/pi-package.sh` (optionally followed by a Nix system such as `x86_64-linux`) to check the current install-lock evaluation and workspace packing commands without building. `App Packages CI` runs this regression check before building changed packages.
- `codex` installs the complete official `codex-package` bundle, including its manifest, helpers, and runtime resources. Its updater tracks bundle hashes for all four platforms. The package's install check validates the manifest, confirms that every archive member is present in the installed output, and checks required executable permissions. Bundled zsh is checked only when present, since upstream removed it in 0.163.0. Run `bash packages/tests/codex-check-package.sh` to check current and legacy bundle validation with local fixtures.
- `App Packages CI` evaluates the nested flake and builds changed packages, including their install checks.
- `notion-cli` tracks the stable version published at `https://ntn.dev/latest.txt`. Its updater downloads all four platform archives, verifies their official SHA-256 checksums, and updates the version and hashes together after every platform succeeds. The `ntn` alias receives the same updates through the direct channel.
- `pantry` packages a `main` build of the Devsisters teamdev.ai CLI, versioned as `main-<12-character commit>` with no releases or tags. It is listed in `update.deny`, so aggregate updates skip it. To bump it manually, run `bash packages/apps/pantry/update.sh`, which pins the build named by the per-platform `latest/<os>-<arch>.json` manifests after confirming all four agree and each binary matches its manifest SHA-256.
- `gksdud` is manually updated. It has no `update.sh`, so aggregate updates skip it. Moving a package into this catalog does not enable automatic updates.
- Package outputs follow each platform's catalog. The Linux build job skips apps absent from the Linux catalog, including `gksdud`; it does not validate their macOS runtime behavior.
- Changes to the nested catalog, flake, lockfile, shared `packages/lib/` helpers, or either package workflow (`apps-ci.yml` and `apps-update.yml`) select all catalog packages for CI builds. A package-local change selects that package. Parent configuration inputs, outputs, package-set wiring, and root lockfile changes do not trigger this independent package CI or widen its build selection: it uses `packages/flake.lock`, not the root lockfile. Parent integration still needs its own targeted Home Manager or NixOS evaluation. The changed-app detector is checked with `bash packages/tests/detect-changed-apps.sh`.
