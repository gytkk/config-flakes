#!/usr/bin/env bash
set -euo pipefail

SOURCE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_ROOT=$(mktemp -d)
trap 'rm -rf "$TEST_ROOT"' EXIT
APP_DIR="$TEST_ROOT/apps/omp"
export FIXTURE_SOURCE="$TEST_ROOT/source"
export FIXTURE_TAG="v99.1.2"
export FIXTURE_REV="1111111111111111111111111111111111111111"
export FIXTURE_DOWNLOAD_FAILURE=0 FIXTURE_PARSE_FAILURE=0
export FIXTURE_DRAFT=false FIXTURE_PRERELEASE=false
REAL_NIX_INSTANTIATE=$(command -v nix-instantiate)
export REAL_NIX_INSTANTIATE
mkdir -p "$APP_DIR" "$TEST_ROOT/bin" "$FIXTURE_SOURCE/nix" "$FIXTURE_SOURCE/packages/coding-agent"
cp "$SOURCE_ROOT/apps/omp/update.sh" "$APP_DIR/"
printf '{ "version": "99.1.2" }\n' > "$FIXTURE_SOURCE/packages/coding-agent/package.json"
for file in flake.nix flake.lock nix/package.nix; do
  touch "$FIXTURE_SOURCE/$file"
done

cat > "$TEST_ROOT/bin/curl" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[ "${!#}" = 'https://api.github.com/repos/can1357/oh-my-pi/releases/latest' ]
jq -n --arg tag "$FIXTURE_TAG" --argjson draft "$FIXTURE_DRAFT" --argjson prerelease "$FIXTURE_PRERELEASE" \
  '{tag_name: $tag, draft: $draft, prerelease: $prerelease}'
EOF
cat > "$TEST_ROOT/bin/nix" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[ "$*" = 'flake metadata --no-write-lock-file --json github:can1357/oh-my-pi/v99.1.2' ]
[ "$FIXTURE_DOWNLOAD_FAILURE" = 0 ]
jq -n --arg path "$FIXTURE_SOURCE" --arg rev "$FIXTURE_REV" '{path: $path, locked: {rev: $rev}}'
EOF
cat > "$TEST_ROOT/bin/nix-instantiate" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[ "$FIXTURE_PARSE_FAILURE" = 0 ]
exec "$REAL_NIX_INSTANTIATE" "$@"
EOF
chmod +x "$TEST_ROOT/bin/"*
export PATH="$TEST_ROOT/bin:$PATH"

reset_package() {
  cp "$SOURCE_ROOT/apps/omp/package.nix" "$APP_DIR/package.nix"
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

[ -x "$SOURCE_ROOT/apps/omp/update.sh" ]
jq -e '(.update.deny // [] | index("omp")) == null' "$SOURCE_ROOT/settings.json" >/dev/null
rg -Fq 'omp = callPackage ./apps/omp/package.nix' "$SOURCE_ROOT/default.nix"
echo 'PASS: OMP is registered and eligible for aggregate updates'

reset_package
bash "$APP_DIR/update.sh"
cp "$TEST_ROOT/original.nix" "$TEST_ROOT/expected.nix"
perl -0pi -e 's/version = "[^"]+";/version = "99.1.2";/; s/rev = "[^"]+";/rev = "1111111111111111111111111111111111111111";/' "$TEST_ROOT/expected.nix"
cmp "$TEST_ROOT/expected.nix" "$APP_DIR/package.nix"
echo 'PASS: update changes only the version and immutable revision'

reset_package
current=$(rg -m1 -o 'version = "([^"]+)";' --replace '$1' "$APP_DIR/package.nix")
FIXTURE_TAG="v$current" bash "$APP_DIR/update.sh"
cmp "$TEST_ROOT/original.nix" "$APP_DIR/package.nix"
echo 'PASS: current version is unchanged without fetching the flake'

for tag in invalid v99.1.2-beta.1 ''; do
  reset_package
  FIXTURE_TAG="$tag" assert_failure
done
reset_package
FIXTURE_DRAFT=true assert_failure
FIXTURE_PRERELEASE=true assert_failure
echo 'PASS: invalid tags, drafts, and prereleases preserve the package'

reset_package
FIXTURE_DOWNLOAD_FAILURE=1 assert_failure
FIXTURE_PARSE_FAILURE=1 assert_failure
echo 'PASS: fetch and parse failures preserve the package and clean up candidates'

for revision in main 1234 ''; do
  reset_package
  FIXTURE_REV="$revision" assert_failure
done
echo 'PASS: unresolved or malformed revisions preserve the package'

reset_package
printf '{ "version": "99.1.1" }\n' > "$FIXTURE_SOURCE/packages/coding-agent/package.json"
assert_failure
printf '{ "version": "99.1.2" }\n' > "$FIXTURE_SOURCE/packages/coding-agent/package.json"
echo 'PASS: mismatched release version preserves the package'

for file in flake.nix flake.lock nix/package.nix; do
  reset_package
  rm "$FIXTURE_SOURCE/$file"
  assert_failure
  touch "$FIXTURE_SOURCE/$file"
done
echo 'PASS: missing upstream packaging files preserve the package'
