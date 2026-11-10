#!/usr/bin/env bash
set -euo pipefail

SOURCE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_ROOT=$(mktemp -d)
trap 'rm -rf "$TEST_ROOT"' EXIT

check_package() {
  bash "$SOURCE_ROOT/apps/codex/check-package.sh" "$output" "$archive" "$version" "$target"
}

assert_failure() {
  local expected=$1
  if check_package > "$TEST_ROOT/check.log" 2>&1; then
    echo "FAIL: checker accepted $expected" >&2
    exit 1
  fi
  if ! rg -F -- "$expected" "$TEST_ROOT/check.log"; then
    cat "$TEST_ROOT/check.log" >&2
    echo "FAIL: checker did not report $expected" >&2
    exit 1
  fi
}

for version in 0.163.0 0.162.1; do
  for target in aarch64-apple-darwin x86_64-apple-darwin x86_64-unknown-linux-musl aarch64-unknown-linux-musl; do
    source_dir="$TEST_ROOT/$version-$target/source"
    output="$TEST_ROOT/$version-$target/output"
    archive="$TEST_ROOT/$version-$target/bundle.tar.gz"
    mkdir -p "$source_dir" "$output"
    jq -n --arg version "$version" --arg target "$target" '{
      layoutVersion: 1, version: $version, target: $target, variant: "codex",
      entrypoint: "bin/codex", resourcesDir: "codex-resources", pathDir: "codex-path"
    }' > "$source_dir/codex-package.json"

    executables=(bin/codex bin/codex-code-mode-host codex-path/rg)
    if [[ "$target" == *-linux-* ]]; then
      executables+=(codex-resources/bwrap)
    fi
    if [ "$version" = 0.162.1 ]; then
      executables+=(codex-resources/zsh/bin/zsh)
    fi
    for executable in "${executables[@]}"; do
      mkdir -p "$(dirname "$source_dir/$executable")"
      printf '#!/bin/sh\nexit 0\n' > "$source_dir/$executable"
      chmod +x "$source_dir/$executable"
    done
    mkdir -p "$source_dir/codex-resources/voice"
    printf '{}\n' > "$source_dir/codex-resources/voice/manifest.json"
    tar -czf "$archive" -C "$source_dir" .
    tar -xzf "$archive" -C "$output"
    check_package
  done
done
echo 'PASS: current and legacy bundles pass on all supported targets'

for executable in "${executables[@]}"; do
  chmod -x "$output/$executable"
  assert_failure "not executable: $executable"
  chmod +x "$output/$executable"
done
echo 'PASS: core helpers and bundled legacy zsh must remain executable'

for member in codex-resources/zsh/bin/zsh codex-resources/voice/manifest.json; do
  rm "$output/$member"
  assert_failure "missing bundle member: ./$member"
  cp -p "$source_dir/$member" "$output/$member"
done
echo 'PASS: every source archive member must remain installed'

printf '{}\n' > "$output/codex-package.json"
assert_failure 'missing or incompatible codex-package.json'
echo 'PASS: incompatible manifests are rejected'
