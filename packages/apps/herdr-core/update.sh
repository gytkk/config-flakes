#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGE_NIX="$SCRIPT_DIR/package.nix"
RELEASES="https://releases.herdr.dev/core"

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

manifest=$(curl -fsSL --retry 3 "$RELEASES/latest.json")
latest=$(jq -er '.version' <<<"$manifest")
[[ "$latest" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "Invalid stable Core version: $latest"
current=$(rg -m 1 -o 'version = "([^"]+)";' --replace '$1' "$PACKAGE_NIX")
if [ "$latest" = "$current" ]; then
  echo "Already at latest version: $latest"
  exit 0
fi

echo "Updating $current -> $latest"
workdir=$(mktemp -d "$SCRIPT_DIR/.update.XXXXXX")
trap 'rm -rf "$workdir"' EXIT
candidate="$workdir/package.nix"
cp "$PACKAGE_NIX" "$candidate"

for system in aarch64-darwin x86_64-darwin aarch64-linux x86_64-linux; do
  target=$(rg -A 2 -F "    $system = {" "$PACKAGE_NIX" | rg -m 1 -o 'target = "([^"]+)";' --replace '$1')
  url="$RELEASES/$latest/herdr-core-$target.tar.gz"
  manifest_url=$(jq -er --arg target "$target" '.targets[$target].url' <<<"$manifest")
  [ "$manifest_url" = "$url" ] || fail "Unexpected release URL for $system: $manifest_url"
  checksum=$(jq -er --arg target "$target" '.targets[$target].sha256' <<<"$manifest")
  [[ "$checksum" =~ ^[0-9a-fA-F]{64}$ ]] || fail "Invalid checksum for $system"
  expected=$(nix hash convert --hash-algo sha256 --to sri "$checksum")
  archive=$(nix store prefetch-file --json "$url")
  hash=$(jq -er '.hash' <<<"$archive")
  [ "$hash" = "$expected" ] || fail "Release checksum mismatch for $system"

  SYSTEM="$system" HASH="$hash" perl -0pi -e '
    s/(\Q$ENV{SYSTEM}\E = \{\s+target = "[^"]+";\s+hash = ")[^"]+/$1 . $ENV{HASH}/e == 1
      or die "Expected one hash for $ENV{SYSTEM}\n";
  ' "$candidate"
done

CURRENT="$current" LATEST="$latest" perl -0pi -e '
  s/version = "\Q$ENV{CURRENT}\E";/version = "$ENV{LATEST}";/g == 1
    or die "Expected one package version\n";
' "$candidate"
mv "$candidate" "$PACKAGE_NIX"
echo "Updated herdr-core to $latest"
