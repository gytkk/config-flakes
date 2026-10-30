#!/usr/bin/env bash
set -euo pipefail

SOURCE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_ROOT=$(mktemp -d)
trap 'rm -rf "$TEST_ROOT"' EXIT
APP_DIR="$TEST_ROOT/pi"
export FIXTURE_SOURCE="$TEST_ROOT/source"
export FIXTURE_TAG="v99.1.2"
export FIXTURE_DOWNLOAD_FAILURE="0"
mkdir -p "$APP_DIR" "$TEST_ROOT/bin" "$FIXTURE_SOURCE/nix" "$FIXTURE_SOURCE/packages/coding-agent/install-lock"
cp "$SOURCE_ROOT/apps/pi/update.sh" "$APP_DIR/update.sh"
printf '{ "version": "99.1.2" }\n' > "$FIXTURE_SOURCE/packages/coding-agent/package.json"
for file in nix/package.nix nix/model-catalog.json packages/coding-agent/install-lock/package-lock.json package-lock.json; do
  touch "$FIXTURE_SOURCE/$file"
done

cat > "$TEST_ROOT/bin/curl" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[ "${!#}" = 'https://api.github.com/repos/earendil-works/pi/releases/latest' ]
jq -n --arg tag "$FIXTURE_TAG" '{tag_name: $tag}'
EOF
cat > "$TEST_ROOT/bin/nix" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[ "$*" = 'store prefetch-file --unpack --json https://github.com/earendil-works/pi/archive/refs/tags/v99.1.2.tar.gz' ]
[ "$FIXTURE_DOWNLOAD_FAILURE" = 0 ]
jq -n --arg path "$FIXTURE_SOURCE" '{hash: "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=", storePath: $path}'
EOF
chmod +x "$TEST_ROOT/bin/curl" "$TEST_ROOT/bin/nix"
export PATH="$TEST_ROOT/bin:$PATH"

reset_package() {
  cp "$SOURCE_ROOT/apps/pi/package.nix" "$APP_DIR/package.nix"
  cp "$APP_DIR/package.nix" "$TEST_ROOT/original.nix"
}

assert_failure() {
  if bash "$APP_DIR/update.sh" > "$TEST_ROOT/output" 2>&1; then
    echo 'FAIL: updater accepted invalid release metadata' >&2
    exit 1
  fi
  cmp "$TEST_ROOT/original.nix" "$APP_DIR/package.nix"
  if compgen -G "$APP_DIR/.package.nix.*" >/dev/null; then
    echo 'FAIL: updater left a temporary candidate' >&2
    exit 1
  fi
}

reset_package
bash "$APP_DIR/update.sh"
cp "$TEST_ROOT/original.nix" "$TEST_ROOT/expected.nix"
perl -0pi -e 's/version = "[^"]+";/version = "99.1.2";/; s/sha256 = "[^"]+";/sha256 = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";/' "$TEST_ROOT/expected.nix"
cmp "$TEST_ROOT/expected.nix" "$APP_DIR/package.nix"
echo 'PASS: update changes only the upstream version and source hash'

reset_package
current=$(rg -m1 -o 'version = "([^"]+)";' --replace '$1' "$APP_DIR/package.nix")
FIXTURE_TAG="v$current" bash "$APP_DIR/update.sh"
cmp "$TEST_ROOT/original.nix" "$APP_DIR/package.nix"
echo 'PASS: current version is unchanged without prefetching'

for tag in invalid v99.1.2-beta.1 ''; do
  reset_package
  FIXTURE_TAG="$tag" assert_failure
done
echo 'PASS: invalid and prerelease tags preserve the package'

reset_package
FIXTURE_DOWNLOAD_FAILURE=1 assert_failure
echo 'PASS: download failure preserves the package'

reset_package
printf '{ "version": "99.1.1" }\n' > "$FIXTURE_SOURCE/packages/coding-agent/package.json"
assert_failure
printf '{ "version": "99.1.2" }\n' > "$FIXTURE_SOURCE/packages/coding-agent/package.json"
echo 'PASS: mismatched release version preserves the package'

for file in nix/package.nix nix/model-catalog.json packages/coding-agent/install-lock/package-lock.json package-lock.json; do
  reset_package
  rm "$FIXTURE_SOURCE/$file"
  assert_failure
  touch "$FIXTURE_SOURCE/$file"
done
echo 'PASS: missing upstream packaging files preserve the package'
