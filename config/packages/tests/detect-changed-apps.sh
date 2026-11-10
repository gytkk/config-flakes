#!/usr/bin/env bash
set -euo pipefail

SOURCE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_ROOT="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$TEST_ROOT"' EXIT

REPO="$TEST_ROOT/repo"
PACKAGES_ROOT="$REPO/config/packages"
APP_ROOT="$PACKAGES_ROOT/apps"
DETECT="$PACKAGES_ROOT/scripts/detect-changed-apps.sh"

commit() {
  git -C "$REPO" add -A
  git -C "$REPO" \
    -c commit.gpgsign=false \
    -c core.hooksPath=/dev/null \
    -c user.name="App package test" \
    -c user.email="app-package-test@example.invalid" \
    commit -q -m "$1"
  git -C "$REPO" rev-parse HEAD
}

assert_apps() {
  local description="$1"
  local expected="$2"
  local base_sha="$3"
  local head_sha="${4:-HEAD}"
  local actual

  actual="$("$DETECT" "$base_sha" "$head_sha")"
  if ! jq -e --argjson expected "$expected" '. == $expected' <<<"$actual" >/dev/null; then
    printf 'FAIL: %s\n  expected: %s\n  actual:   %s\n' "$description" "$expected" "$actual" >&2
    exit 1
  fi
  printf 'PASS: %s\n' "$description"
}

mkdir -p "$APP_ROOT"/{alpha,beta} "$PACKAGES_ROOT"/{lib,scripts}
cp "$SOURCE_ROOT/scripts/detect-changed-apps.sh" "$DETECT"
chmod +x "$DETECT"
printf '{ }\n' > "$APP_ROOT/alpha/package.nix"
printf '{ }\n' > "$APP_ROOT/beta/package.nix"
printf '{ }\n' > "$PACKAGES_ROOT/default.nix"
printf '{ }\n' > "$PACKAGES_ROOT/flake.nix"
printf '# fixtures\n' > "$PACKAGES_ROOT/README.md"
printf '{ value = 1; }\n' > "$PACKAGES_ROOT/lib/helper.nix"

git -C "$REPO" init -q
initial_sha="$(commit 'initial fixtures')"

printf '{ changed = true; }\n' > "$APP_ROOT/alpha/package.nix"
alpha_sha="$(commit 'change alpha')"
assert_apps "one app change selects that app" '["alpha"]' "$initial_sha" "$alpha_sha"

printf '{ value = 2; }\n' > "$PACKAGES_ROOT/lib/helper.nix"
lib_sha="$(commit 'change shared helper')"
assert_apps "shared helper change rebuilds every app" '["alpha","beta"]' "$alpha_sha" "$lib_sha"

printf '{ catalog = true; }\n' > "$PACKAGES_ROOT/default.nix"
catalog_sha="$(commit 'change catalog')"
assert_apps "catalog change rebuilds every app" '["alpha","beta"]' "$lib_sha" "$catalog_sha"

printf '{ changed = true; }\n' > "$PACKAGES_ROOT/flake.nix"
flake_sha="$(commit 'change nested flake')"
assert_apps "nested flake change rebuilds every app" '["alpha","beta"]' "$catalog_sha" "$flake_sha"

printf '{ }\n' > "$PACKAGES_ROOT/flake.lock"
lock_sha="$(commit 'change nested lockfile')"
assert_apps "nested lockfile change rebuilds every app" '["alpha","beta"]' "$flake_sha" "$lock_sha"

printf '# documentation only\n' >> "$PACKAGES_ROOT/README.md"
readme_sha="$(commit 'change readme')"
assert_apps "README-only change selects no apps" '[]' "$lock_sha" "$readme_sha"

assert_apps "missing baseline rebuilds every app" '["alpha","beta"]' ""
assert_apps "invalid baseline rebuilds every app" '["alpha","beta"]' "not-a-commit"

mkdir -p "$REPO/config/lib"
parent_sha="$readme_sha"
for path in flake.nix flake.lock config/flake.nix config/outputs.nix config/lib/pkgs.nix; do
  printf '{ }\n' > "$REPO/$path"
  next_sha="$(commit "change $path")"
  assert_apps "$path does not affect the standalone package flake" '[]' "$parent_sha" "$next_sha"
  parent_sha="$next_sha"
done

printf '{ changed = true; }\n' > "$REPO/flake.lock"
printf '{ changed = true; }\n' > "$APP_ROOT/beta/package.nix"
mixed_sha="$(commit 'change parent lockfile and beta')"
assert_apps "parent changes do not widen a package change" '["beta"]' "$parent_sha" "$mixed_sha"

mkdir -p "$REPO/.github/workflows"
printf '# changed workflow\n' > "$REPO/.github/workflows/apps-ci.yml"
workflow_sha="$(commit 'change package build workflow')"
assert_apps "package build workflow change rebuilds every app" '["alpha","beta"]' "$mixed_sha" "$workflow_sha"

rm "$APP_ROOT/alpha/package.nix"
deleted_sha="$(commit 'delete alpha')"
assert_apps "deleted package rebuilds all remaining apps" '["beta"]' "$workflow_sha" "$deleted_sha"
