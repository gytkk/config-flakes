#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGE_NIX="$SCRIPT_DIR/package.nix"

GCS_BUCKET="https://storage.googleapis.com/claude-code-dist-86c565f3-f756-42ad-8dfa-d59b1c096819/claude-code-releases"

LATEST=$(curl -fsSL "$GCS_BUCKET/latest")

if [ -z "$LATEST" ] || [ "$LATEST" = "null" ]; then
  echo "ERROR: Failed to fetch latest version"
  exit 1
fi

CURRENT=$(rg -m 1 'version = ' "$PACKAGE_NIX" | sed 's/.*"\(.*\)".*/\1/')

if [ "$LATEST" = "$CURRENT" ]; then
  echo "Already at latest version: $LATEST"
  exit 0
fi

echo "Updating $CURRENT -> $LATEST"

MANIFEST=$(curl -fsSL "$GCS_BUCKET/$LATEST/manifest.json")

declare -A PLATFORM_SUFFIX=(
  ["aarch64-darwin"]="darwin-arm64"
  ["x86_64-darwin"]="darwin-x64"
  ["x86_64-linux"]="linux-x64"
  ["aarch64-linux"]="linux-arm64"
)

declare -A HASHES
for system in aarch64-darwin x86_64-darwin x86_64-linux aarch64-linux; do
  suffix="${PLATFORM_SUFFIX[$system]}"
  hex_hash=$(echo "$MANIFEST" | jq -r ".platforms[\"$suffix\"].checksum")

  if [ -z "$hex_hash" ] || [ "$hex_hash" = "null" ]; then
    echo "ERROR: No checksum found for $suffix"
    exit 1
  fi

  sri_hash=$(nix hash convert --to sri "sha256:$hex_hash")
  HASHES[$system]="$sri_hash"
  echo "  $system: $sri_hash"
done

sed -i "s/version = \"$CURRENT\"/version = \"$LATEST\"/" "$PACKAGE_NIX"

for system in aarch64-darwin x86_64-darwin x86_64-linux aarch64-linux; do
  old_hash=$(rg -A3 "\"$system\"" "$PACKAGE_NIX" | rg 'hash = ' | sed 's/.*"\(.*\)".*/\1/')
  new_hash="${HASHES[$system]}"
  if [ -n "$old_hash" ] && [ -n "$new_hash" ]; then
    sed -i "s|$old_hash|$new_hash|" "$PACKAGE_NIX"
  fi
done

echo "Updated package.nix to version $LATEST"
