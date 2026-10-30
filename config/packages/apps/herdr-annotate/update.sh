#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGES_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PACKAGE_NIX="$SCRIPT_DIR/package.nix"
SOURCES_JSON="$SCRIPT_DIR/sources.json"
PLANNOTATOR_TUI_NIX="$SCRIPT_DIR/plannotator-tui.nix"
REPOSITORY="plannotator/herdr-annotate"
PLANNOTATOR_REPOSITORY="plannotator/plannotator-tui"

tmp_dir="$(mktemp -d)"
cleanup() {
  find "$tmp_dir" -type f -delete
  rmdir "$tmp_dir"
}
trap cleanup EXIT

github_api() {
  curl -fsSL --retry 3 \
    -H "Accept: application/vnd.github+json" \
    -H "X-GitHub-Api-Version: 2022-11-28" \
    "$1"
}

main_commit=$(github_api "https://api.github.com/repos/$REPOSITORY/commits/main")
revision=$(jq -er '.sha' <<<"$main_commit")
revision_date=$(jq -er '.commit.committer.date | split("T")[0]' <<<"$main_commit")

raw_base="https://raw.githubusercontent.com/$REPOSITORY/$revision"
plugin_version=$(curl -fsSL --retry 3 "$raw_base/herdr-plugin.toml" | rg -m1 '^version = ' | sed 's/.*"\(.*\)".*/\1/')
herdr_annotate_version=$(curl -fsSL --retry 3 "$raw_base/herdr-annotate.version" | tr -d '[:space:]')
plannotator_tui_version=$(curl -fsSL --retry 3 "$raw_base/plannotator-tui.version" | tr -d '[:space:]')

for value in "$plugin_version" "$herdr_annotate_version" "$plannotator_tui_version"; do
  if [ -z "$value" ]; then
    echo "ERROR: Upstream version metadata is incomplete" >&2
    exit 1
  fi
done

package_version=$(rg -m1 'version = ' "$PACKAGE_NIX" | sed 's/.*"\(.*\)".*/\1/')
source_url="https://github.com/$REPOSITORY/archive/$revision.tar.gz"
source_hash_base32=$(nix-prefetch-url --unpack "$source_url")
source_hash=$(nix hash convert --hash-algo sha256 --to sri "$source_hash_base32")

plannotator_source=$(nix store prefetch-file --unpack --json "https://github.com/$PLANNOTATOR_REPOSITORY/archive/refs/tags/v$plannotator_tui_version.tar.gz")
plannotator_source_hash=$(jq -er '.hash' <<<"$plannotator_source")

current_plannotator_version=$(jq -er '.plannotatorTui.version' "$SOURCES_JSON")
current_plannotator_source_hash=$(jq -er '.plannotatorTui.sourceHash' "$SOURCES_JSON")
current_plannotator_cargo_hash=$(jq -er '.plannotatorTui.cargoHash' "$SOURCES_JSON")
if [ "$plannotator_tui_version" = "$current_plannotator_version" ] \
  && [ "$plannotator_source_hash" = "$current_plannotator_source_hash" ] \
  && [ "$current_plannotator_cargo_hash" != "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=" ]; then
  cargo_hash="$current_plannotator_cargo_hash"
else
  jq \
    --arg plannotatorVersion "$plannotator_tui_version" \
    --arg plannotatorSourceHash "$plannotator_source_hash" \
    '.plannotatorTui = {
      version: $plannotatorVersion,
      sourceHash: $plannotatorSourceHash,
      cargoHash: ""
    }' \
    "$SOURCES_JSON" > "$tmp_dir/cargo-sources.json"

  set +e
  cargo_output=$(nix build --no-link --impure --expr "
    let
      flake = builtins.getFlake \"path:$PACKAGES_ROOT\";
      pkgs = import flake.inputs.nixpkgs { system = builtins.currentSystem; };
      sources = builtins.fromJSON (builtins.readFile \"$tmp_dir/cargo-sources.json\");
    in
      (pkgs.callPackage \"$PLANNOTATOR_TUI_NIX\" { inherit sources; }).cargoDeps.vendorStaging
  " 2>&1)
  cargo_status=$?
  set -e

  if [ "$cargo_status" -eq 0 ]; then
    echo "ERROR: Expected Cargo dependency hash discovery to report a fixed-output hash mismatch" >&2
    exit 1
  fi

  cargo_hash=$(rg -o 'got:[[:space:]]+sha256-[A-Za-z0-9+/=]+' <<<"$cargo_output" | awk '{print $2}' | tail -n 1 || true)
  if [ -z "$cargo_hash" ]; then
    echo "ERROR: Could not determine the Cargo dependency hash" >&2
    echo "$cargo_output" >&2
    exit 1
  fi
fi

targets=(
  aarch64-apple-darwin
  x86_64-apple-darwin
  x86_64-unknown-linux-gnu
  aarch64-unknown-linux-gnu
)

herdr_sums=$(curl -fsSL --retry 3 "https://github.com/$REPOSITORY/releases/download/rust-lite-v$herdr_annotate_version/SHA256SUMS")

release_hash() {
  local sums="$1"
  local asset="$2"
  local hex
  hex=$(rg " $asset\$" <<<"$sums" | awk '{print $1}' || true)
  if [ -z "$hex" ]; then
    echo "ERROR: Release checksums do not list $asset" >&2
    exit 1
  fi
  nix hash convert --hash-algo sha256 --to sri "$hex"
}

herdr_aarch64_darwin=$(release_hash "$herdr_sums" "herdr-annotate-${targets[0]}")
herdr_x8664_darwin=$(release_hash "$herdr_sums" "herdr-annotate-${targets[1]}")
herdr_x8664_linux=$(release_hash "$herdr_sums" "herdr-annotate-${targets[2]}")
herdr_aarch64_linux=$(release_hash "$herdr_sums" "herdr-annotate-${targets[3]}")

jq -n \
  --arg revision "$revision" \
  --arg revisionDate "$revision_date" \
  --arg sourceHash "$source_hash" \
  --arg herdrVersion "$herdr_annotate_version" \
  --arg plannotatorVersion "$plannotator_tui_version" \
  --arg plannotatorSourceHash "$plannotator_source_hash" \
  --arg plannotatorCargoHash "$cargo_hash" \
  --arg herdrAarch64Darwin "$herdr_aarch64_darwin" \
  --arg herdrX8664Darwin "$herdr_x8664_darwin" \
  --arg herdrX8664Linux "$herdr_x8664_linux" \
  --arg herdrAarch64Linux "$herdr_aarch64_linux" \
  '{
    revision: $revision,
    revisionDate: $revisionDate,
    sourceHash: $sourceHash,
    herdrAnnotate: {
      version: $herdrVersion,
      hashes: {
        "aarch64-apple-darwin": $herdrAarch64Darwin,
        "x86_64-apple-darwin": $herdrX8664Darwin,
        "x86_64-unknown-linux-gnu": $herdrX8664Linux,
        "aarch64-unknown-linux-gnu": $herdrAarch64Linux
      }
    },
    plannotatorTui: {
      version: $plannotatorVersion,
      sourceHash: $plannotatorSourceHash,
      cargoHash: $plannotatorCargoHash
    }
  }' > "$tmp_dir/sources.json"

if cmp -s "$tmp_dir/sources.json" "$SOURCES_JSON" && [ "$plugin_version" = "$package_version" ]; then
  echo "Already current: plugin $plugin_version at $revision"
  exit 0
fi

cp "$PACKAGE_NIX" "$tmp_dir/package.nix"
if [ "$plugin_version" != "$package_version" ]; then
  OLD="$package_version" NEW="$plugin_version" perl -0pi -e 's/version = "\Q$ENV{OLD}\E"/version = "$ENV{NEW}"/' "$tmp_dir/package.nix"
fi

mv "$tmp_dir/package.nix" "$PACKAGE_NIX"
mv "$tmp_dir/sources.json" "$SOURCES_JSON"
echo "Updated plugin $plugin_version to $revision ($revision_date)"
echo "  herdr-annotate: $herdr_annotate_version"
echo "  plannotator-tui: $plannotator_tui_version (source and Cargo dependencies)"
