#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "Usage: $0 SOURCE PATCH_MANIFEST.nix" >&2
  exit 2
fi

source_dir="$1"
manifest="$(cd "$(dirname "$2")" && pwd)/$(basename "$2")"
patches=$(nix-instantiate --eval --strict --json \
  --expr '{ manifest }: map builtins.toString (import manifest)' \
  --argstr manifest "$manifest")
patch_paths=$(jq -r '.[]' <<<"$patches")
if [ -z "$patch_paths" ]; then
  exit 0
fi

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
cp -R "$source_dir" "$scratch/source"
chmod -R u+w "$scratch/source"

while IFS= read -r patch_path; do
  echo "Checking $(basename "$patch_path")"
  # Keep Nix's default fuzz tolerance, but never reverse an already applied patch.
  if ! patch --batch --forward -p1 -d "$scratch/source" < "$patch_path"; then
    echo "ERROR: $patch_path does not apply in the order declared by $manifest" >&2
    exit 1
  fi
done <<<"$patch_paths"
