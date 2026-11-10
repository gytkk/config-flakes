#!/usr/bin/env bash
set -euo pipefail

SOURCE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_ROOT=$(mktemp -d)
trap 'rm -rf "$TEST_ROOT"' EXIT
mkdir -p "$TEST_ROOT/scripts"
cp "$SOURCE_ROOT/scripts/update-all.sh" "$TEST_ROOT/scripts/"
export TEST_LOG="$TEST_ROOT/calls"
export FAIL_APP=""

for app in alpha beta herdr manual; do
  mkdir -p "$TEST_ROOT/apps/$app"
  printf '{ }\n' > "$TEST_ROOT/apps/$app/package.nix"
  [ "$app" != manual ] || continue
  cat > "$TEST_ROOT/apps/$app/update.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
app=$(basename "$(dirname "$0")")
echo "$app" >> "$TEST_LOG"
if [ "$app" = "$FAIL_APP" ]; then
  exit 17
fi
EOF
  chmod +x "$TEST_ROOT/apps/$app/update.sh"
done

printf '{"update":{"deny":[]}}\n' > "$TEST_ROOT/settings.json"

assert_calls() {
  local expected="$1"
  if [ "$(cat "$TEST_LOG")" != "$expected" ]; then
    echo "FAIL: Expected calls '$expected', got '$(cat "$TEST_LOG")'" >&2
    exit 1
  fi
}

: > "$TEST_LOG"
bash "$TEST_ROOT/scripts/update-all.sh"
assert_calls $'alpha\nbeta\nherdr'
echo 'PASS: all enabled apps run and apps without an executable updater are skipped'

: > "$TEST_LOG"
if FAIL_APP=beta bash "$TEST_ROOT/scripts/update-all.sh"; then
  echo 'FAIL: updater failure was hidden' >&2
  exit 1
else
  status=$?
fi
if [ "$status" -ne 17 ]; then
  echo "FAIL: Expected updater status 17, got $status" >&2
  exit 1
fi
assert_calls $'alpha\nbeta'
echo 'PASS: updater failures propagate and stop later updaters'

printf '{"update":{"deny":["herdr"]}}\n' > "$TEST_ROOT/settings.json"
: > "$TEST_LOG"
bash "$TEST_ROOT/scripts/update-all.sh"
assert_calls $'alpha\nbeta'
echo 'PASS: denied apps are skipped'

printf '{ invalid json\n' > "$TEST_ROOT/settings.json"
: > "$TEST_LOG"
if bash "$TEST_ROOT/scripts/update-all.sh" > "$TEST_ROOT/error" 2>&1; then
  echo 'FAIL: malformed settings were ignored' >&2
  exit 1
fi
assert_calls ''
echo 'PASS: malformed policy stops before running any updater'

printf '{"update":{"deny":[]}}\n' > "$TEST_ROOT/settings.json"
for argument in direct review typo; do
  : > "$TEST_LOG"
  if bash "$TEST_ROOT/scripts/update-all.sh" "$argument" > "$TEST_ROOT/error" 2>&1; then
    echo "FAIL: unexpected argument '$argument' was accepted" >&2
    exit 1
  fi
  assert_calls ''
done
: > "$TEST_LOG"
if bash "$TEST_ROOT/scripts/update-all.sh" direct review > "$TEST_ROOT/error" 2>&1; then
  echo 'FAIL: extra arguments were accepted' >&2
  exit 1
fi
assert_calls ''
echo 'PASS: unexpected and extra arguments are rejected before running any updater'
