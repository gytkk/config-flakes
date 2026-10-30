#!/usr/bin/env bash
set -euo pipefail

SOURCE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_ROOT=$(mktemp -d)
trap 'rm -rf "$TEST_ROOT"' EXIT
APP_DIR="$TEST_ROOT/notion-cli"
mkdir -p "$APP_DIR" "$TEST_ROOT/bin" "$TEST_ROOT/tmp"
cp "$SOURCE_ROOT/apps/notion-cli/update.sh" "$APP_DIR/update.sh"
export FIXTURE_LOG="$TEST_ROOT/requests"
export FIXTURE_LATEST="v99.1.2"
export FIXTURE_FAILURE=""
export FIXTURE_TARGET="aarch64-apple-darwin"
export TMPDIR="$TEST_ROOT/tmp"

cat > "$TEST_ROOT/bin/curl" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
url="${!#}"
printf 'curl %s\n' "$url" >> "$FIXTURE_LOG"
if [ "$url" = https://ntn.dev/latest.txt ]; then
  printf '%s\n' "$FIXTURE_LATEST"
  exit 0
fi
case "$url" in
  "https://ntn.dev/releases/v99.1.2/ntn-"*.tar.gz.sha256) ;;
  *) printf 'Unexpected checksum URL: %s\n' "$url" >&2; exit 2 ;;
esac
filename="${url##*/}"
filename="${filename%.sha256}"
target="${filename#ntn-}"
target="${target%.tar.gz}"
if [ "$target" = "$FIXTURE_TARGET" ]; then
  case "$FIXTURE_FAILURE" in
    checksum-download) exit 22 ;;
    malformed-checksum) printf 'missing checksum\n'; exit 0 ;;
    wrong-filename) filename="ntn-wrong-target.tar.gz" ;;
  esac
fi
case "$target" in
  x86_64-unknown-linux-musl) digit=1 ;;
  aarch64-unknown-linux-musl) digit=2 ;;
  x86_64-apple-darwin) digit=3 ;;
  aarch64-apple-darwin) digit=4 ;;
  *) printf 'Unexpected target: %s\n' "$target" >&2; exit 2 ;;
esac
printf '%064d  %s\n' 0 "$filename" | tr 0 "$digit"
EOF

cat > "$TEST_ROOT/bin/nix-prefetch-url" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
url="${!#}"
printf 'prefetch %s\n' "$url" >> "$FIXTURE_LOG"
case "$url" in
  "https://ntn.dev/releases/v99.1.2/ntn-"*.tar.gz) ;;
  *) printf 'Unexpected archive URL: %s\n' "$url" >&2; exit 2 ;;
esac
target="${url##*/ntn-}"
target="${target%.tar.gz}"
if [ "$target" = "$FIXTURE_TARGET" ]; then
  case "$FIXTURE_FAILURE" in
    archive-download) exit 1 ;;
    mismatch) printf '%064d\n' 0 | tr 0 9; exit 0 ;;
  esac
fi
case "$target" in
  x86_64-unknown-linux-musl) digit=1 ;;
  aarch64-unknown-linux-musl) digit=2 ;;
  x86_64-apple-darwin) digit=3 ;;
  aarch64-apple-darwin) digit=4 ;;
  *) printf 'Unexpected target: %s\n' "$target" >&2; exit 2 ;;
esac
printf '%064d\n' 0 | tr 0 "$digit"
EOF
chmod +x "$TEST_ROOT/bin/curl" "$TEST_ROOT/bin/nix-prefetch-url"
export PATH="$TEST_ROOT/bin:$PATH"

reset_package() {
  cp "$SOURCE_ROOT/apps/notion-cli/package.nix" "$APP_DIR/package.nix"
  cp "$APP_DIR/package.nix" "$TEST_ROOT/original.nix"
  : > "$FIXTURE_LOG"
}

assert_clean() {
  if compgen -G "$APP_DIR/.update.*" > /dev/null || [ -n "$(ls -A "$TEST_ROOT/tmp")" ]; then
    echo 'FAIL: updater left temporary files behind' >&2
    exit 1
  fi
}

assert_failure() {
  if bash "$APP_DIR/update.sh" > "$TEST_ROOT/output" 2>&1; then
    printf 'FAIL: updater accepted %s\n' "$FIXTURE_FAILURE" >&2
    exit 1
  fi
  if ! cmp -s "$TEST_ROOT/original.nix" "$APP_DIR/package.nix"; then
    printf 'FAIL: %s changed package metadata\n' "$FIXTURE_FAILURE" >&2
    exit 1
  fi
  assert_clean
}

reset_package
bash "$APP_DIR/update.sh"
expected="$TEST_ROOT/expected.nix"
cp "$TEST_ROOT/original.nix" "$expected"
original_version=$(rg -m 1 -o 'version = "([^"]+)";' --replace '$1' "$expected")
sed -i.bak "s/version = \"$original_version\";/version = \"99.1.2\";/" "$expected"
old_hashes=()
while IFS= read -r hash; do
  old_hashes+=("$hash")
done < <(rg -o 'hash = "([^"]+)";' --replace '$1' "$TEST_ROOT/original.nix")
for index in 0 1 2 3; do
  digit=$((index + 1))
  hex=$(printf '%064d' 0 | tr 0 "$digit")
  sri=$(nix hash convert --hash-algo sha256 --to sri "$hex")
  old_hash="${old_hashes[$index]}"
  sed -i.bak "s|$old_hash|$sri|g" "$expected"
done
if ! cmp -s "$expected" "$APP_DIR/package.nix"; then
  echo 'FAIL: successful update did not change exactly the version and four hashes' >&2
  diff -u "$expected" "$APP_DIR/package.nix" >&2
  exit 1
fi
printf 'curl https://ntn.dev/latest.txt\n' > "$TEST_ROOT/expected-requests"
for target in x86_64-unknown-linux-musl aarch64-unknown-linux-musl x86_64-apple-darwin aarch64-apple-darwin; do
  url="https://ntn.dev/releases/v99.1.2/ntn-$target.tar.gz"
  printf 'curl %s.sha256\nprefetch %s\n' "$url" "$url" >> "$TEST_ROOT/expected-requests"
done
if ! cmp -s "$TEST_ROOT/expected-requests" "$FIXTURE_LOG"; then
  echo 'FAIL: update requested unexpected release assets or platform order' >&2
  diff -u "$TEST_ROOT/expected-requests" "$FIXTURE_LOG" >&2
  exit 1
fi
assert_clean
echo 'PASS: successful update pins all four verified archives'

reset_package
FIXTURE_LATEST="v$original_version" bash "$APP_DIR/update.sh"
cmp -s "$TEST_ROOT/original.nix" "$APP_DIR/package.nix"
if [ "$(cat "$FIXTURE_LOG")" != 'curl https://ntn.dev/latest.txt' ]; then
  echo 'FAIL: current version fetched release archives' >&2
  exit 1
fi
assert_clean
echo 'PASS: current version leaves package unchanged and skips archives'

for latest in '' v99.1.2-beta.1 invalid; do
  reset_package
  FIXTURE_LATEST="$latest" FIXTURE_FAILURE=invalid-version assert_failure
done
echo 'PASS: invalid latest versions are rejected without metadata changes'

for failure in checksum-download malformed-checksum wrong-filename archive-download mismatch; do
  reset_package
  FIXTURE_FAILURE="$failure" assert_failure
  if ! rg -Fq "$FIXTURE_TARGET" "$FIXTURE_LOG"; then
    printf 'FAIL: %s did not reach the final platform\n' "$failure" >&2
    exit 1
  fi
done
echo 'PASS: checksum and download failures on the final platform preserve original metadata'

reset_package
sed -i.bak '/target = "aarch64-apple-darwin";/d' "$APP_DIR/package.nix"
cp "$APP_DIR/package.nix" "$TEST_ROOT/original.nix"
FIXTURE_FAILURE=missing-target assert_failure
echo 'PASS: missing final-platform target preserves original metadata'
