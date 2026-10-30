#!/usr/bin/env bash
set -euo pipefail

SOURCE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHECK_PATCHES="$SOURCE_ROOT/scripts/check-nix-patches.sh"
TEST_ROOT="$(mktemp -d)"
trap 'rm -rf "$TEST_ROOT"' EXIT

check_patches() {
  bash "$CHECK_PATCHES" "$@"
}

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

assert_same() {
  local description="$1"
  local actual="$2"
  local expected="$3"

  cmp -s "$actual" "$expected" || fail "$description"
  printf 'PASS: %s\n' "$description"
}

assert_fails() {
  local description="$1"
  shift

  if "$@"; then
    fail "$description"
  fi
  printf 'PASS: %s\n' "$description"
}

write_manifest() {
  local path="$1"
  shift

  {
    printf '[\n'
    for patch_path in "$@"; do
      printf '  ./%s\n' "$(basename "$patch_path")"
    done
    printf ']\n'
  } > "$path"
}

FIXTURES="$TEST_ROOT/fixtures"
mkdir -p "$FIXTURES/source" "$FIXTURES/final-source"
printf 'first\nsecond\nthird\n' > "$FIXTURES/source/story.txt"
printf 'final\nsecond\nthird\n' > "$FIXTURES/final-source/story.txt"
cp "$FIXTURES/source/story.txt" "$FIXTURES/original-story.txt"
cp "$FIXTURES/final-source/story.txt" "$FIXTURES/original-final-story.txt"

write_manifest "$FIXTURES/empty-patches.nix"
check_patches "$FIXTURES/source" "$FIXTURES/empty-patches.nix"
assert_same "an empty patch list leaves the fetched source unchanged" \
  "$FIXTURES/source/story.txt" "$FIXTURES/original-story.txt"

cat > "$FIXTURES/first.patch" <<'EOF'
--- a/story.txt
+++ b/story.txt
@@ -1,3 +1,3 @@
-first
+intermediate
 second
 third
EOF

cat > "$FIXTURES/second.patch" <<'EOF'
--- a/story.txt
+++ b/story.txt
@@ -1,3 +1,3 @@
-intermediate
+final
 second
 third
EOF

cat > "$FIXTURES/failing-third.patch" <<'EOF'
--- a/story.txt
+++ b/story.txt
@@ -1,3 +1,3 @@
-missing
+never-written
 second
 third
EOF

write_manifest "$FIXTURES/ordered-patches.nix" \
  "$FIXTURES/first.patch" \
  "$FIXTURES/second.patch"
check_patches "$FIXTURES/source" "$FIXTURES/ordered-patches.nix"
assert_same "sequential patches leave the fetched source unchanged" \
  "$FIXTURES/source/story.txt" "$FIXTURES/original-story.txt"

write_manifest "$FIXTURES/reversed-patches.nix" \
  "$FIXTURES/second.patch" \
  "$FIXTURES/first.patch"
assert_fails "dependent patches fail in reverse order" \
  check_patches "$FIXTURES/source" "$FIXTURES/reversed-patches.nix"
assert_same "a reverse-order failure leaves the fetched source unchanged" \
  "$FIXTURES/source/story.txt" "$FIXTURES/original-story.txt"

write_manifest "$FIXTURES/failing-patches.nix" \
  "$FIXTURES/first.patch" \
  "$FIXTURES/second.patch" \
  "$FIXTURES/failing-third.patch"
assert_fails "a failed later patch is reported" \
  check_patches "$FIXTURES/source" "$FIXTURES/failing-patches.nix"
assert_same "a later-patch failure leaves the fetched source unchanged" \
  "$FIXTURES/source/story.txt" "$FIXTURES/original-story.txt"

assert_fails "already-applied patches are not reversed" \
  check_patches "$FIXTURES/final-source" "$FIXTURES/ordered-patches.nix"
assert_same "a reverse-application rejection leaves the fetched source unchanged" \
  "$FIXTURES/final-source/story.txt" "$FIXTURES/original-final-story.txt"

mkdir -p "$FIXTURES/fuzzy-source"
printf 'different heading\nshared context\nold value\ntrailing context\n' > "$FIXTURES/fuzzy-source/fuzzy.txt"
cp "$FIXTURES/fuzzy-source/fuzzy.txt" "$FIXTURES/original-fuzzy.txt"
cat > "$FIXTURES/fuzzy.patch" <<'EOF'
--- a/fuzzy.txt
+++ b/fuzzy.txt
@@ -1,4 +1,4 @@
 expected heading
 shared context
-old value
+new value
 trailing context
EOF
write_manifest "$FIXTURES/fuzzy-patches.nix" "$FIXTURES/fuzzy.patch"
check_patches "$FIXTURES/fuzzy-source" "$FIXTURES/fuzzy-patches.nix"
assert_same "default patch fuzz leaves the fetched source unchanged" \
  "$FIXTURES/fuzzy-source/fuzzy.txt" "$FIXTURES/original-fuzzy.txt"
