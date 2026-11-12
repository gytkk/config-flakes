#!/usr/bin/env bash
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
module_root="$repo_root/modules/pi"

if [[ -z ${PI_TEST_NODE_MODULES:-} ]]; then
  pi_binary=$(command -v pi) || {
    echo "Pi is not on PATH. Set PI_TEST_NODE_MODULES to its node_modules directory." >&2
    exit 1
  }
  pi_binary=$(realpath "$pi_binary")
  PI_TEST_NODE_MODULES="$(dirname "$(dirname "$pi_binary")")/lib/pi/node_modules"
fi
PI_TEST_NODE_MODULES=$(realpath "$PI_TEST_NODE_MODULES")
package="$PI_TEST_NODE_MODULES/@earendil-works/pi-coding-agent/package.json"
if [[ ! -f $package ]]; then
  echo "Pi package not found at $package. Set PI_TEST_NODE_MODULES to the installed runtime's node_modules directory." >&2
  exit 1
fi
expected=$(rg --only-matching --replace '$1' '^  version = "([^"]+)";' "$repo_root/packages/apps/pi/package.nix")
actual=$(jq -er '.version' "$package")
if [[ $actual != "$expected" ]]; then
  echo "Pi version mismatch: repository pins $expected, test runtime is $actual." >&2
  exit 1
fi

# Real copies keep Bun's module resolution inside the isolated test tree.
temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT
export PI_TEST_MODULE_CONFIG="$temporary/module-config.json"
nix eval --impure --json --file "$module_root/tests/module-config.nix" > "$PI_TEST_MODULE_CONFIG"
cp -R "$module_root" "$temporary/pi"
ln -s "$PI_TEST_NODE_MODULES" "$temporary/pi/node_modules"
export PI_CODING_AGENT_DIR="$temporary/agent"
mkdir -p "$PI_CODING_AGENT_DIR"

tests=("$temporary/pi/tests")
if (( $# )); then
  tests=()
  for name in "$@"; do
    if [[ $name == */* || $name != *.test.ts || ! -f $module_root/tests/$name ]]; then
      echo "Expected a test filename from modules/pi/tests, got: $name" >&2
      exit 1
    fi
    tests+=("$temporary/pi/tests/$name")
  done
fi
printf 'Testing with Pi %s from %s\n' "$actual" "$PI_TEST_NODE_MODULES"
bun test "${tests[@]}"
