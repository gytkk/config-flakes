#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGE_NIX="$SCRIPT_DIR/package.nix"
GITHUB_REPOSITORY="can1357/oh-my-pi"

for cmd in curl jq nix nix-instantiate perl rg mktemp; do
  command -v "$cmd" >/dev/null 2>&1 || {
    echo "ERROR: missing required command: $cmd" >&2
    exit 1
  }
done

release=$(curl -fsSL --retry 3 "https://api.github.com/repos/$GITHUB_REPOSITORY/releases/latest")
tag=$(jq -er 'select(.draft == false and .prerelease == false) | .tag_name' <<<"$release")
if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "ERROR: Unexpected stable release tag: $tag" >&2
  exit 1
fi
latest="${tag#v}"
current=$(rg -m1 -o 'version = "([^"]+)";' --replace '$1' "$PACKAGE_NIX")
if [ "$latest" = "$current" ]; then
  echo "Already at latest version: $latest"
  exit 0
fi

metadata=$(nix flake metadata --no-write-lock-file --json "github:$GITHUB_REPOSITORY/$tag")
revision=$(jq -er '.locked.rev' <<<"$metadata")
source_path=$(jq -er '.path' <<<"$metadata")
if [[ ! "$revision" =~ ^[0-9a-f]{40}$ ]]; then
  echo "ERROR: Release $tag did not resolve to a full Git revision: $revision" >&2
  exit 1
fi
source_version=$(jq -er '.version' "$source_path/packages/coding-agent/package.json")
if [ "$source_version" != "$latest" ]; then
  echo "ERROR: Release $tag declares package version $source_version" >&2
  exit 1
fi
for file in flake.nix flake.lock nix/package.nix; do
  if [ ! -f "$source_path/$file" ]; then
    echo "ERROR: Release $tag lacks required upstream packaging file: $file" >&2
    exit 1
  fi
done

candidate=$(mktemp "$SCRIPT_DIR/.package.nix.XXXXXX")
trap 'rm -f "$candidate"' EXIT
cp "$PACKAGE_NIX" "$candidate"
NEW_VERSION="$latest" SOURCE_REV="$revision" perl -0pi -e '
  s/version = "[^"]+";/version = "$ENV{NEW_VERSION}";/;
  s/rev = "[^"]+";/rev = "$ENV{SOURCE_REV}";/;
' "$candidate"
nix-instantiate --parse "$candidate" >/dev/null
mv "$candidate" "$PACKAGE_NIX"
echo "Updated upstream OMP package $current -> $latest ($revision)"
