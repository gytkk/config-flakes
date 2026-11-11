#!/usr/bin/env bash
set -euo pipefail

SOURCE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_ROOT=$(mktemp -d)
trap 'rm -rf "$TEST_ROOT"' EXIT
mkdir -p "$TEST_ROOT/app" "$TEST_ROOT/bin"
cp "$SOURCE_ROOT/apps/herdr-core/update.sh" "$TEST_ROOT/app/update.sh"
cp "$SOURCE_ROOT/apps/herdr-core/package.nix" "$TEST_ROOT/original.nix"
export REAL_NIX
REAL_NIX=$(command -v nix)
export FIXTURE_VERSION=99.1.2 FIXTURE_FAILURE=""
export FIXTURE_HASH="sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="

cat > "$TEST_ROOT/bin/curl" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
jq -n --arg version "$FIXTURE_VERSION" --arg failure "$FIXTURE_FAILURE" '
  {version: $version, targets: (
    ["macos-aarch64", "macos-x86_64", "linux-aarch64", "linux-x86_64"]
    | map(. as $target | {key: $target, value: {
        url: ("https://releases.herdr.dev/core/" + $version + "/herdr-core-" + $target + ".tar.gz"),
        sha256: ("0" * 64)
      }}) | from_entries
  )}
  | if $failure == "missing-target" then del(.targets["linux-x86_64"])
    elif $failure == "invalid-hash" then .targets["linux-x86_64"].sha256 = "bad"
    elif $failure == "wrong-url" then .targets["linux-x86_64"].url = "https://example.org/unexpected"
    else . end'
EOF
cat > "$TEST_ROOT/bin/nix" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [ "$1" != store ]; then
  exec "$REAL_NIX" "$@"
fi
if [[ "$*" == *herdr-core-linux-x86_64.tar.gz ]]; then
  case "$FIXTURE_FAILURE" in
    download) exit 1 ;;
    mismatch) printf '{"hash":"sha256-//////////////////////////////////////////8="}\n'; exit 0 ;;
  esac
fi
jq -n --arg hash "$FIXTURE_HASH" '{hash: $hash}'
EOF
chmod +x "$TEST_ROOT/bin/curl" "$TEST_ROOT/bin/nix"
export PATH="$TEST_ROOT/bin:$PATH"

reset_package() {
  cp "$TEST_ROOT/original.nix" "$TEST_ROOT/app/package.nix"
}
assert_clean() {
  if compgen -G "$TEST_ROOT/app/.update.*" >/dev/null; then
    echo 'FAIL: updater left staging files' >&2
    exit 1
  fi
}

reset_package
bash "$TEST_ROOT/app/update.sh"
version=$(rg -m 1 -o 'version = "([^"]+)";' --replace '$1' "$TEST_ROOT/app/package.nix")
[ "$version" = "$FIXTURE_VERSION" ]
hashes=$(rg -o 'hash = "([^"]+)";' --replace '$1' "$TEST_ROOT/app/package.nix")
[ "$hashes" = "$(printf '%s\n%s\n%s\n%s' "$FIXTURE_HASH" "$FIXTURE_HASH" "$FIXTURE_HASH" "$FIXTURE_HASH")" ]
assert_clean
echo 'PASS: successful update replaces the version and all verified platform hashes'

cp "$TEST_ROOT/app/package.nix" "$TEST_ROOT/updated.nix"
bash "$TEST_ROOT/app/update.sh"
cmp "$TEST_ROOT/updated.nix" "$TEST_ROOT/app/package.nix"
assert_clean
echo 'PASS: current version is unchanged'

for failure in missing-target invalid-hash wrong-url download mismatch; do
  reset_package
  if FIXTURE_FAILURE="$failure" bash "$TEST_ROOT/app/update.sh" > "$TEST_ROOT/output" 2>&1; then
    echo "FAIL: accepted $failure" >&2
    exit 1
  fi
  cmp "$TEST_ROOT/original.nix" "$TEST_ROOT/app/package.nix"
  assert_clean
done
echo 'PASS: final-platform failures preserve the complete original pin'

reset_package
if FIXTURE_VERSION=99.1.2-beta.1 bash "$TEST_ROOT/app/update.sh" > "$TEST_ROOT/output" 2>&1; then
  echo 'FAIL: accepted prerelease' >&2
  exit 1
fi
cmp "$TEST_ROOT/original.nix" "$TEST_ROOT/app/package.nix"
assert_clean
echo 'PASS: prereleases are rejected without changing the pin'
