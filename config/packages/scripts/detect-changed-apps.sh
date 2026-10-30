#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGES_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
APP_ROOT="$PACKAGES_ROOT/apps"
REPO_ROOT="$(git -C "$PACKAGES_ROOT" rev-parse --show-toplevel)"
PACKAGES_ROOT_REL="${PACKAGES_ROOT#"$REPO_ROOT"/}"
APP_ROOT_REL="${APP_ROOT#"$REPO_ROOT"/}"
BASE_SHA="${1:-}"
HEAD_SHA="${2:-HEAD}"

list_apps() {
  local package_file=""
  for package_file in "$APP_ROOT"/*/package.nix; do
    [ -f "$package_file" ] || continue
    basename "$(dirname "$package_file")"
  done | sort
}

emit_json() {
  jq -Rsc 'split("\n") | map(select(length > 0))'
}

if [ -z "$BASE_SHA" ] || ! git -C "$REPO_ROOT" rev-parse --verify "$BASE_SHA^{commit}" >/dev/null 2>&1; then
  list_apps | emit_json
  exit 0
fi

changed_apps=()
rebuild_all=0

while IFS= read -r path; do
  case "$path" in
    flake.nix | flake.lock | config/outputs.nix | config/lib/pkgs.nix | .github/workflows/apps-ci.yml | "$PACKAGES_ROOT_REL/default.nix" | "$PACKAGES_ROOT_REL/flake.nix" | "$PACKAGES_ROOT_REL/flake.lock" | "$PACKAGES_ROOT_REL/lib/"*)
      rebuild_all=1
      ;;
    "$APP_ROOT_REL/"*)
      relative_path="${path#"$APP_ROOT_REL/"}"
      app_name="${relative_path%%/*}"
      if [ -f "$APP_ROOT/$app_name/package.nix" ]; then
        changed_apps+=("$app_name")
      elif git -C "$REPO_ROOT" cat-file -e "$BASE_SHA:$APP_ROOT_REL/$app_name/package.nix" 2>/dev/null; then
        rebuild_all=1
      fi
      ;;
  esac
done < <(git -C "$REPO_ROOT" diff --name-only "$BASE_SHA" "$HEAD_SHA")

if [ "$rebuild_all" -eq 1 ]; then
  list_apps | emit_json
elif [ "${#changed_apps[@]}" -eq 0 ]; then
  printf '[]\n'
else
  printf '%s\n' "${changed_apps[@]}" | sort -u | emit_json
fi
