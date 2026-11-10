#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGE_NIX="$SCRIPT_DIR/package.nix"
BASE_URL="https://devsisters-vibe-static-public.s3.ap-northeast-2.amazonaws.com/pantry/cli"
SYSTEMS=(x86_64-linux aarch64-linux x86_64-darwin aarch64-darwin)

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

CURRENT=$(rg -m 1 -o 'version = "([^"]+)";' --replace '$1' "$PACKAGE_NIX") || fail "Failed to read version from $PACKAGE_NIX"

# Pantry publishes one build per main commit. Every platform manifest must point at the same build.
declare -A ASSETS EXPECTED_HASHES
LATEST=""
for system in "${SYSTEMS[@]}"; do
  asset=$(rg -A 2 -F "    $system = {" "$PACKAGE_NIX" | rg -m 1 -o 'asset = "([^"]+)";' --replace '$1') \
    || fail "Failed to read asset for $system"
  manifest=$(curl -fsSL --retry 3 "$BASE_URL/latest/$asset.json") || fail "Failed to fetch the latest manifest for $asset"
  version=$(jq -er '.version' <<< "$manifest") || fail "Missing version in the $asset manifest"
  platform=$(jq -er '.platform' <<< "$manifest") || fail "Missing platform in the $asset manifest"
  url=$(jq -er '.url' <<< "$manifest") || fail "Missing url in the $asset manifest"
  hex_hash=$(jq -er '.sha256' <<< "$manifest") || fail "Missing sha256 in the $asset manifest"

  [[ "$version" =~ ^main-[0-9a-f]{12}$ ]] || fail "Invalid pantry version for $asset: $version"
  [ "$platform" = "$asset" ] || fail "Manifest platform $platform does not match $asset"
  [ "$url" = "$BASE_URL/$version/$asset/pantry" ] || fail "Unexpected download URL for $asset: $url"
  [[ "$hex_hash" =~ ^[0-9a-f]{64}$ ]] || fail "Invalid sha256 for $asset"
  [ -z "$LATEST" ] || [ "$version" = "$LATEST" ] \
    || fail "Platform manifests disagree on the latest version: $LATEST and $version ($asset)"

  LATEST="$version"
  ASSETS[$system]="$asset"
  EXPECTED_HASHES[$system]=$(nix hash convert --hash-algo sha256 --to sri "$hex_hash")
done

if [ "$LATEST" = "$CURRENT" ]; then
  echo "Already at latest version: $LATEST"
  exit 0
fi

echo "Updating $CURRENT -> $LATEST"

WORKDIR=$(mktemp -d "$SCRIPT_DIR/.update.XXXXXX")
trap 'rm -rf "$WORKDIR"' EXIT
CANDIDATE="$WORKDIR/package.nix"
cp "$PACKAGE_NIX" "$CANDIDATE"

for system in "${SYSTEMS[@]}"; do
  url="$BASE_URL/$LATEST/${ASSETS[$system]}/pantry"

  echo "Fetching hash for $system..."
  binary_hash=$(nix-prefetch-url --type sha256 "$url") || fail "Failed to fetch binary for $system"
  sri_hash=$(nix hash convert --to sri "sha256:$binary_hash")
  [ "$sri_hash" = "${EXPECTED_HASHES[$system]}" ] || fail "Manifest checksum mismatch for $system"

  SYSTEM="$system" HASH="$sri_hash" perl -0pi -e '
    my $count = s/(\Q$ENV{SYSTEM}\E = \{\s+asset = "[^"]+";\s+hash = ")[^"]+/$1 . $ENV{HASH}/e;
    die "Expected one hash for $ENV{SYSTEM}\n" unless $count == 1;
  ' "$CANDIDATE"
  echo "  $system: $sri_hash"
done

CURRENT="$CURRENT" LATEST="$LATEST" perl -0pi -e '
  my $count = s/version = "\Q$ENV{CURRENT}\E";/version = "$ENV{LATEST}";/g;
  die "Expected one package version\n" unless $count == 1;
' "$CANDIDATE"
mv "$CANDIDATE" "$PACKAGE_NIX"

echo "Updated package.nix to version $LATEST"
