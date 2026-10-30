#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGE_NIX="$SCRIPT_DIR/package.nix"
GITHUB_REPOSITORY="earendil-works/pi"

for cmd in curl jq nix nix-instantiate perl rg mktemp; do
  command -v "$cmd" >/dev/null 2>&1 || {
    echo "ERROR: missing required command: $cmd" >&2
    exit 1
  }
done

release=$(curl -fsSL --retry 3 "https://api.github.com/repos/$GITHUB_REPOSITORY/releases/latest")
tag=$(jq -er '.tag_name' <<<"$release")
if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "ERROR: Unexpected release tag: $tag" >&2
  exit 1
fi
latest="${tag#v}"
current=$(rg -m1 -o 'version = "([^"]+)";' --replace '$1' "$PACKAGE_NIX")
if [ "$latest" = "$current" ]; then
  echo "Already at latest version: $latest"
  exit 0
fi

source=$(nix store prefetch-file --unpack --json "https://github.com/$GITHUB_REPOSITORY/archive/refs/tags/$tag.tar.gz")
source_hash=$(jq -er '.hash' <<<"$source")
source_path=$(jq -er '.storePath' <<<"$source")
source_version=$(jq -er '.version' "$source_path/packages/coding-agent/package.json")
if [ "$source_version" != "$latest" ]; then
  echo "ERROR: Release $tag declares package version $source_version" >&2
  exit 1
fi
for file in nix/package.nix nix/model-catalog.json packages/coding-agent/install-lock/package-lock.json package-lock.json; do
  if [ ! -f "$source_path/$file" ]; then
    echo "ERROR: Release $tag lacks required upstream packaging file: $file" >&2
    exit 1
  fi
done

candidate=$(mktemp "$SCRIPT_DIR/.package.nix.XXXXXX")
trap 'rm -f "$candidate"' EXIT
cp "$PACKAGE_NIX" "$candidate"
NEW_VERSION="$latest" SOURCE_HASH="$source_hash" perl -0pi -e '
  s/version = "[^"]+";/version = "$ENV{NEW_VERSION}";/;
  s/sha256 = "[^"]+";/sha256 = "$ENV{SOURCE_HASH}";/;
' "$candidate"
nix-instantiate --parse "$candidate" >/dev/null
mv "$candidate" "$PACKAGE_NIX"
echo "Updated upstream Pi package $current -> $latest"
