#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGE_NIX="$SCRIPT_DIR/package.nix"

RELEASE=$(curl -fsSL --retry 3 https://api.github.com/repos/steipete/CodexBar/releases/latest)
LATEST=$(jq -er '.tag_name | sub("^v"; "")' <<<"$RELEASE")
CURRENT=$(rg -m1 'version = ' "$PACKAGE_NIX" | sed 's/.*"\(.*\)".*/\1/')

if [ "$LATEST" = "$CURRENT" ]; then
  echo "Already at latest version: $LATEST"
  exit 0
fi

echo "Updating $CURRENT -> $LATEST"

declare -A PLATFORM_TARGET=(
  ["aarch64-darwin"]="macos-arm64"
  ["x86_64-darwin"]="macos-x86_64"
  ["x86_64-linux"]="linux-musl-x86_64"
  ["aarch64-linux"]="linux-musl-aarch64"
)

declare -A HASHES
for system in aarch64-darwin x86_64-darwin x86_64-linux aarch64-linux; do
  target="${PLATFORM_TARGET[$system]}"
  asset="CodexBarCLI-v${LATEST}-${target}.tar.gz"
  hex_hash=$(jq -er --arg asset "$asset" \
    '.assets[] | select(.name == $asset) | .digest | sub("^sha256:"; "")' <<<"$RELEASE")
  HASHES[$system]=$(nix hash convert --hash-algo sha256 --to sri "$hex_hash")
  echo "  $system: ${HASHES[$system]}"
done

OLD="$CURRENT" NEW="$LATEST" perl -0pi -e 's/version = "\Q$ENV{OLD}\E"/version = "$ENV{NEW}"/' "$PACKAGE_NIX"

for system in aarch64-darwin x86_64-darwin x86_64-linux aarch64-linux; do
  old_hash=$(rg -A3 "\"$system\"" "$PACKAGE_NIX" | rg -m1 'hash = ' | sed 's/.*"\(.*\)".*/\1/')
  new_hash="${HASHES[$system]}"
  OLD="$old_hash" NEW="$new_hash" perl -0pi -e 's/\Q$ENV{OLD}\E/$ENV{NEW}/' "$PACKAGE_NIX"
done

echo "Updated package.nix to version $LATEST"
