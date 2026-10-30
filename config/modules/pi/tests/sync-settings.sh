#!/usr/bin/env bash
set -euo pipefail

SYNC="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/files/sync-settings.sh"
TEST_ROOT=$(mktemp -d)
trap 'rm -rf "$TEST_ROOT"' EXIT
common="$TEST_ROOT/common.json"
local_settings="$TEST_ROOT/agent/settings.json"
printf '{"theme":"system","defaultProvider":"openai","subagents":{"defaultModel":"openai/gpt-6.1-sol"}}\n' > "$common"

bash "$SYNC" "$common" "$local_settings"
[ ! -L "$local_settings" ]
cmp "$local_settings" <(jq . "$common")
[ "$(find "$local_settings" -perm 0600 | wc -l | tr -d ' ')" = 1 ]
echo 'PASS: new installation gets common settings in a private regular file'

jq '. + {deviceId:"machine-a", trackingId:"local-tracking", lastChangelogVersion:"1.1.0", localSetting:true, subagents:{localOverride:true}}' "$common" > "$TEST_ROOT/legacy.json"
cp "$TEST_ROOT/legacy.json" "$TEST_ROOT/legacy-original.json"
rm "$local_settings"
ln -s "$TEST_ROOT/legacy.json" "$local_settings"
bash "$SYNC" "$common" "$local_settings"
[ ! -L "$local_settings" ]
cmp "$TEST_ROOT/legacy.json" "$TEST_ROOT/legacy-original.json"
jq -e '.deviceId == "machine-a" and .trackingId == "local-tracking" and .lastChangelogVersion == "1.1.0" and .localSetting == true and .subagents == {defaultModel:"openai/gpt-6.1-sol"}' "$local_settings" >/dev/null
echo 'PASS: symlink migration preserves local metadata without changing its source'

cp "$local_settings" "$TEST_ROOT/local-original.json"
bash "$SYNC" "$common" "$local_settings"
cmp "$local_settings" "$TEST_ROOT/local-original.json"
echo 'PASS: repeated synchronization is idempotent'

jq '.deviceId = "machine-b"' "$local_settings" > "$TEST_ROOT/other.json"
bash "$SYNC" "$common" "$TEST_ROOT/other.json"
jq -e '.deviceId == "machine-b"' "$TEST_ROOT/other.json" >/dev/null
jq -e '.deviceId == "machine-a"' "$local_settings" >/dev/null
echo 'PASS: separate installations retain separate identities'

assert_failure() {
  if bash "$SYNC" "$1" "$2" > "$TEST_ROOT/output" 2>&1; then
    echo 'FAIL: synchronization accepted invalid settings' >&2
    exit 1
  fi
  cmp "$local_settings" "$TEST_ROOT/local-original.json"
  if compgen -G "${local_settings}.??????" >/dev/null; then
    echo 'FAIL: synchronization left a temporary file' >&2
    exit 1
  fi
}
for field in deviceId trackingId lastChangelogVersion; do
  jq --arg field "$field" '.[$field] = "must-not-share"' "$common" > "$TEST_ROOT/invalid-common.json"
  assert_failure "$TEST_ROOT/invalid-common.json" "$local_settings"
done
printf 'invalid JSON\n' > "$TEST_ROOT/invalid-common.json"
assert_failure "$TEST_ROOT/invalid-common.json" "$local_settings"
echo 'PASS: common settings reject local identities and malformed JSON without changes'

printf 'invalid JSON\n' > "$TEST_ROOT/invalid-local.json"
cp "$TEST_ROOT/invalid-local.json" "$TEST_ROOT/invalid-original.json"
assert_failure "$common" "$TEST_ROOT/invalid-local.json"
cmp "$TEST_ROOT/invalid-local.json" "$TEST_ROOT/invalid-original.json"
for value in '[]' 'null'; do
  printf '%s\n' "$value" > "$TEST_ROOT/invalid-local.json"
  cp "$TEST_ROOT/invalid-local.json" "$TEST_ROOT/invalid-original.json"
  assert_failure "$common" "$TEST_ROOT/invalid-local.json"
  cmp "$TEST_ROOT/invalid-local.json" "$TEST_ROOT/invalid-original.json"
done
ln -s "$TEST_ROOT/missing.json" "$TEST_ROOT/broken.json"
assert_failure "$common" "$TEST_ROOT/broken.json"
[ -L "$TEST_ROOT/broken.json" ]
echo 'PASS: invalid local settings and broken symlinks are preserved for diagnosis'
