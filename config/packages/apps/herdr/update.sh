#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGE_NIX="$SCRIPT_DIR/package.nix"
MANIFEST_URL="https://herdr.dev/latest.json"

MANIFEST=$(curl -fsSL --retry 3 "$MANIFEST_URL")
LATEST=$(jq -er '.version // empty' <<<"$MANIFEST")
CURRENT=$(rg -m1 'version = ' "$PACKAGE_NIX" | sed 's/.*"\(.*\)".*/\1/')

if [ "$LATEST" = "$CURRENT" ]; then
  echo "Already at latest version: $LATEST"
  exit 0
fi

echo "Updating $CURRENT -> $LATEST"

if [[ ! "$LATEST" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "ERROR: Unexpected stable version: $LATEST" >&2
  exit 1
fi

source_json=$(nix store prefetch-file --unpack --json "https://github.com/herdrdev/herdr/archive/refs/tags/v$LATEST.tar.gz")
source_hash=$(jq -er '.hash' <<<"$source_json")
source_path=$(jq -er '.storePath' <<<"$source_json")

for required in Cargo.lock vendor/libghostty-vt/build.zig.zon vendor/libghostty-vt/build.zig.zon.nix; do
  if [ ! -f "$source_path/$required" ]; then
    echo "ERROR: Herdr v$LATEST is missing $required; review the source build configuration" >&2
    exit 1
  fi
done

if ! rg -q '^\s*\.minimum_zig_version\s*=\s*"0\.16\.0"' "$source_path/vendor/libghostty-vt/build.zig.zon"; then
  echo "ERROR: Herdr v$LATEST needs a different Zig toolchain; review package.nix and zig.nix" >&2
  exit 1
fi

bash "$SCRIPT_DIR/../../scripts/check-nix-patches.sh" "$source_path" "$SCRIPT_DIR/patches.nix"

tmp_file=$(mktemp "$SCRIPT_DIR/package.nix.XXXXXX")
trap 'rm -f "$tmp_file"' EXIT
cp "$PACKAGE_NIX" "$tmp_file"
NEW_VERSION="$LATEST" SOURCE_HASH="$source_hash" perl -0pi -e '
  s/version = "[^"]+";/version = "$ENV{NEW_VERSION}";/ == 1 or die "Expected one version\n";
  s/\bsha256 = "[^"]+";/sha256 = "$ENV{SOURCE_HASH}";/ == 1 or die "Expected one source hash\n";
' "$tmp_file"
mv "$tmp_file" "$PACKAGE_NIX"

echo "Updated Herdr source to version $LATEST"
