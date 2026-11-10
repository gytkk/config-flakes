#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APPS_DIR="$ROOT_DIR/apps"
SETTINGS="$ROOT_DIR/settings.json"
if [ "$#" -ne 0 ]; then
  echo "Usage: $0" >&2
  exit 2
fi

DENY_LIST=""
if [ -f "$SETTINGS" ]; then
  DENY_LIST=$(jq -r '.update.deny // [] | .[]' "$SETTINGS")
fi

for app_dir in "$APPS_DIR"/*; do
  [ -d "$app_dir" ] || continue
  [ -f "$app_dir/package.nix" ] || continue

  app_name=$(basename "$app_dir")
  update_script="$app_dir/update.sh"

  if [ ! -x "$update_script" ]; then
    echo "Skipping $app_name: no update.sh"
    continue
  fi

  if printf '%s\n' "$DENY_LIST" | rg -qx "$app_name"; then
    echo "Skipping $app_name: denied in settings.json"
    continue
  fi

  echo "Running $app_name update"
  "$update_script"
done
