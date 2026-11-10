#!/usr/bin/env bash
set -euo pipefail

SOURCE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_ROOT=$(mktemp -d)
trap 'rm -rf "$TEST_ROOT"' EXIT
APP_DIR="$TEST_ROOT/apps/hunk"
export FIXTURE_SOURCE="$TEST_ROOT/source"
export FIXTURE_TAG="v99.1.2"
export FIXTURE_REV="1111111111111111111111111111111111111111"
export FIXTURE_DOWNLOAD_FAILURE=0
export FIXTURE_DRAFT=false FIXTURE_PRERELEASE=false
mkdir -p "$APP_DIR" "$TEST_ROOT/bin" "$FIXTURE_SOURCE/nix" "$FIXTURE_SOURCE/packages/hunk"
cp "$SOURCE_ROOT/apps/hunk/update.sh" "$APP_DIR/"
printf '{ "version": "99.1.2" }\n' > "$FIXTURE_SOURCE/packages/hunk/package.json"
for file in flake.nix flake.lock nix/package.nix nix/bun.lock.nix; do
  touch "$FIXTURE_SOURCE/$file"
done

cat > "$TEST_ROOT/bin/curl" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[ "${!#}" = 'https://api.github.com/repos/modem-dev/hunk/releases/latest' ]
jq -n --arg tag "$FIXTURE_TAG" --argjson draft "$FIXTURE_DRAFT" --argjson prerelease "$FIXTURE_PRERELEASE" \
  '{tag_name: $tag, draft: $draft, prerelease: $prerelease}'
EOF
cat > "$TEST_ROOT/bin/nix" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[ "$*" = 'flake metadata --no-write-lock-file --json github:modem-dev/hunk/v99.1.2' ]
[ "$FIXTURE_DOWNLOAD_FAILURE" = 0 ]
jq -n --arg path "$FIXTURE_SOURCE" --arg rev "$FIXTURE_REV" '{path: $path, locked: {rev: $rev}}'
EOF
chmod +x "$TEST_ROOT/bin/"*
export PATH="$TEST_ROOT/bin:$PATH"

reset_package() {
  cat > "$APP_DIR/package.nix" <<'EOF'
{ version = "1.0.0"; rev = "2222222222222222222222222222222222222222"; keep = "unchanged"; }
EOF
  cp "$APP_DIR/package.nix" "$TEST_ROOT/original.nix"
}

assert_failure() {
  if bash "$APP_DIR/update.sh" > "$TEST_ROOT/output" 2>&1; then
    echo 'FAIL: updater accepted invalid release metadata or a failed operation' >&2
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
nix-instantiate --eval --strict --json "$APP_DIR/package.nix" | jq -e \
  '. == {version: "99.1.2", rev: "1111111111111111111111111111111111111111", keep: "unchanged"}' >/dev/null
echo 'PASS: update replaces the version and immutable revision together'

reset_package
FIXTURE_TAG=v1.0.0 FIXTURE_DOWNLOAD_FAILURE=1 bash "$APP_DIR/update.sh"
cmp "$TEST_ROOT/original.nix" "$APP_DIR/package.nix"
echo 'PASS: current release needs no source fetch or package change'

for tag in invalid v99.1.2-beta.1 ''; do
  reset_package
  FIXTURE_TAG="$tag" assert_failure
done
reset_package
FIXTURE_DRAFT=true assert_failure
FIXTURE_PRERELEASE=true assert_failure
FIXTURE_DOWNLOAD_FAILURE=1 assert_failure
for revision in main 1234 ''; do
  FIXTURE_REV="$revision" assert_failure
done
echo 'PASS: invalid releases and failed fetches preserve the package'

printf '{ "version": "99.1.1" }\n' > "$FIXTURE_SOURCE/packages/hunk/package.json"
assert_failure
printf '{ "version": "99.1.2" }\n' > "$FIXTURE_SOURCE/packages/hunk/package.json"
for file in flake.nix flake.lock nix/package.nix nix/bun.lock.nix; do
  rm "$FIXTURE_SOURCE/$file"
  assert_failure
  touch "$FIXTURE_SOURCE/$file"
done
echo 'PASS: inconsistent or incomplete release sources preserve the package'

printf '{ version = "1.0.0"; rev = "2222222222222222222222222222222222222222"; invalid nix\n' > "$APP_DIR/package.nix"
cp "$APP_DIR/package.nix" "$TEST_ROOT/original.nix"
assert_failure
echo 'PASS: invalid candidate is not published and temporary files are removed'
