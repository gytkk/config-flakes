#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGE_NIX="$SCRIPT_DIR/package.nix"

LATEST=$(curl -s https://api.github.com/repos/vercel-labs/agent-browser/releases/latest \
  | jq -r '.tag_name' | sed 's/^v//')

if [ -z "$LATEST" ] || [ "$LATEST" = "null" ]; then
  echo "ERROR: Failed to fetch latest version from GitHub API"
  exit 1
fi

CURRENT=$(rg -m 1 'version = ' "$PACKAGE_NIX" | sed 's/.*"\(.*\)".*/\1/')

if [ "$LATEST" = "$CURRENT" ]; then
  echo "Already at latest version: $LATEST"
  exit 0
fi

echo "Updating $CURRENT -> $LATEST"

declare -A PLATFORM_SUFFIX=(
  ["aarch64-darwin"]="darwin-arm64"
  ["x86_64-darwin"]="darwin-x64"
  ["x86_64-linux"]="linux-x64"
  ["aarch64-linux"]="linux-arm64"
)

declare -A HASHES
for system in aarch64-darwin x86_64-darwin x86_64-linux aarch64-linux; do
  suffix="${PLATFORM_SUFFIX[$system]}"
  url="https://github.com/vercel-labs/agent-browser/releases/download/v${LATEST}/agent-browser-${suffix}"

  echo "Fetching hash for $system..."
  nix_hash=$(nix-prefetch-url "$url" 2>/dev/null) || {
    echo "ERROR: Failed to fetch hash for $system"
    exit 1
  }

  sri_hash=$(nix hash convert --to sri "sha256:$nix_hash")

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
