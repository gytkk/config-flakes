#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGE_NIX="$SCRIPT_DIR/package.nix"

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

LATEST=$(curl -fsSL --retry 3 https://ntn.dev/latest.txt) || fail "Failed to fetch the latest Notion CLI version"
LATEST="${LATEST#v}"
[[ "$LATEST" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "Invalid Notion CLI version: $LATEST"
CURRENT=$(rg -m 1 -o 'version = "([^"]+)";' --replace '$1' "$PACKAGE_NIX") || fail "Failed to read version from $PACKAGE_NIX"

if [ "$LATEST" = "$CURRENT" ]; then
  echo "Already at latest version: $LATEST"
  exit 0
fi

echo "Updating $CURRENT -> $LATEST"

WORKDIR=$(mktemp -d "$SCRIPT_DIR/.update.XXXXXX")
trap 'rm -rf "$WORKDIR"' EXIT
CANDIDATE="$WORKDIR/package.nix"
cp "$PACKAGE_NIX" "$CANDIDATE"

for system in x86_64-linux aarch64-linux x86_64-darwin aarch64-darwin; do
  target=$(rg -A 2 -F "    $system = {" "$PACKAGE_NIX" | rg -m 1 -o 'target = "([^"]+)";' --replace '$1') \
    || fail "Failed to read release target for $system"
  archive="ntn-$target.tar.gz"
  url="https://ntn.dev/releases/v$LATEST/$archive"
  checksum=$(curl -fsSL --retry 3 "$url.sha256") || fail "Failed to fetch checksum for $system"
  read -r hex_hash checksum_archive <<< "$checksum"
  [[ "$hex_hash" =~ ^[0-9a-fA-F]{64}$ ]] && [ "$checksum_archive" = "$archive" ] \
    || fail "Invalid release checksum for $system"
  expected_hash=$(nix hash convert --hash-algo sha256 --to sri "$hex_hash")

  echo "Fetching hash for $system..."
  archive_hash=$(nix-prefetch-url --type sha256 "$url") || fail "Failed to fetch archive for $system"
  sri_hash=$(nix hash convert --to sri "sha256:$archive_hash")
  [ "$sri_hash" = "$expected_hash" ] || fail "Release checksum mismatch for $system"

  SYSTEM="$system" HASH="$sri_hash" perl -0pi -e '
    my $count = s/(\Q$ENV{SYSTEM}\E = \{\s+target = "[^"]+";\s+hash = ")[^"]+/$1 . $ENV{HASH}/e;
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
