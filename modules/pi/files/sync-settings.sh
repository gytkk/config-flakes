#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "Usage: pi-sync-settings <common-settings.json> <local-settings.json>" >&2
  exit 1
fi
common="$1"
settings="$2"

if ! jq -e '
  type == "object"
  and (has("deviceId") | not)
  and (has("trackingId") | not)
  and (has("lastChangelogVersion") | not)
' "$common" >/dev/null; then
  echo "Pi common settings must be an object without installation-local metadata: $common" >&2
  exit 1
fi

if [ -e "$settings" ] || [ -L "$settings" ]; then
  if ! jq -e 'type == "object"' "$settings" >/dev/null; then
    echo "Pi local settings are not a readable JSON object; leaving them unchanged: $settings" >&2
    exit 1
  fi
fi

mkdir -p "$(dirname "$settings")"
tmp=$(mktemp "${settings}.XXXXXX")
trap 'rm -f "$tmp"' EXIT
if [ -e "$settings" ] || [ -L "$settings" ]; then
  jq --slurpfile common "$common" '. + $common[0]' "$settings" > "$tmp"
else
  jq . "$common" > "$tmp"
fi
chmod 600 "$tmp"

if [ ! -L "$settings" ] && [ -f "$settings" ] && cmp -s "$settings" "$tmp"; then
  chmod 600 "$settings"
else
  mv -f "$tmp" "$settings"
fi
