#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGES_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PACKAGE_NIX="$SCRIPT_DIR/package.nix"
REPOSITORY="kryptamine/herdr-auto-title"

release=$(curl -fsSL --retry 3 "https://api.github.com/repos/$REPOSITORY/releases/latest")
tag=$(jq -er '.tag_name' <<<"$release")
latest="${tag#v}"
current=$(rg -m1 'version = ' "$PACKAGE_NIX" | sed 's/.*"\(.*\)".*/\1/')

if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "ERROR: Unexpected release tag: $tag" >&2
  exit 1
fi

if [ "$latest" = "$current" ]; then
  echo "Already at latest version: $latest"
  exit 0
fi

echo "Updating $current -> $latest"

tmp_dir=$(mktemp -d)
cleanup() {
  find "$tmp_dir" -depth -delete
}
trap cleanup EXIT

source=$(nix store prefetch-file --unpack --json "https://github.com/$REPOSITORY/archive/refs/tags/$tag.tar.gz")
source_hash=$(jq -er '.hash' <<<"$source")
source_path=$(jq -er '.storePath' <<<"$source")
plugin_version=$(rg -m1 '^version = ' "$source_path/herdr-plugin.toml" | sed 's/.*"\(.*\)".*/\1/')
if [ "$plugin_version" != "$latest" ]; then
  echo "ERROR: Release $tag declares plugin version $plugin_version" >&2
  exit 1
fi

cp -R "$source_path" "$tmp_dir/source"
chmod -R u+w "$tmp_dir/source"
GOENV=off GOWORK=off GOFLAGS= GOTOOLCHAIN=local \
  nix shell --inputs-from "$PACKAGES_ROOT" nixpkgs#go \
  --command go -C "$tmp_dir/source" mod vendor
vendor_hash=$(nix hash path "$tmp_dir/source/vendor")

cp "$PACKAGE_NIX" "$tmp_dir/package.nix"
NEW_VERSION="$latest" SOURCE_HASH="$source_hash" VENDOR_HASH="$vendor_hash" \
  perl -0pi -e '
    s/version = "[^"]+";/version = "$ENV{NEW_VERSION}";/;
    s/\bhash = "[^"]+";/hash = "$ENV{SOURCE_HASH}";/;
    s/vendorHash = "[^"]+";/vendorHash = "$ENV{VENDOR_HASH}";/;
  ' "$tmp_dir/package.nix"

mv "$tmp_dir/package.nix" "$PACKAGE_NIX"
echo "Updated package.nix to version $latest"
