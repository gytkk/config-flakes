#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
README="$ROOT_DIR/README.md"
APPS_DIR="$ROOT_DIR/apps"

if [ ! -f "$README" ]; then
  exit 0
fi

if ! rg -q "^## App versions" "$README"; then
  echo "WARNING: '## App versions' section not found in README.md"
  exit 0
fi

section_file=""
output_file=""
trap 'rm -f "$section_file" "$output_file"' EXIT
trap 'exit 1' HUP INT TERM
section_file=$(mktemp "$ROOT_DIR/.README.versions.XXXXXX")

{
  echo "## App versions"
  echo ""
  echo "| App | Version |"
  echo "|-----|---------|"
} > "$section_file"

for app_dir in "$APPS_DIR"/*; do
  pkg="$app_dir/package.nix"
  [ -f "$pkg" ] || continue
  app=$(basename "$app_dir")
  # Package metadata uses one single-line literal assignment, not a Nix expression.
  if ! ver=$(awk '
    /^[ \t]*version[ \t]*=/ {
      count++
      if ($0 !~ /^[ \t]*version[ \t]*=[ \t]*"[[:alnum:]][[:alnum:].+_-]*";[ \t]*(#.*)?$/) {
        invalid = 1
        next
      }
      value = $0
      sub(/^[^"]*"/, "", value)
      sub(/".*$/, "", value)
    }
    END {
      if (count != 1 || invalid) exit 1
      print value
    }
  ' "$pkg"); then
    printf 'ERROR: expected one literal version assignment in %s\n' "$pkg" >&2
    exit 1
  fi
  printf '| %s | %s |\n' "$app" "$ver" >> "$section_file"
done

output_file=$(mktemp "$ROOT_DIR/.README.md.XXXXXX")
cp -p "$README" "$output_file"

awk -v sf="$section_file" '
  /^## App versions/ {
    while ((result = (getline line < sf)) > 0) print line
    if (result < 0) exit 1
    print ""
    skip = 1
    next
  }
  skip && /^## / { skip = 0 }
  skip { next }
  { print }
' "$README" > "$output_file"
mv "$output_file" "$README"
