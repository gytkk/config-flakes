#!/usr/bin/env bash
set -euo pipefail

SOURCE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
system="${1:-$(nix eval --impure --raw --expr builtins.currentSystem)}"

# Force the real install-lock evaluation; a missing workspace override fails here.
metadata=$(nix eval --json "path:$SOURCE_ROOT#packages.$system.pi" --apply '
  package:
  {
    lock = package.npmDeps.packageLock;
    contexts = builtins.attrNames (builtins.getContext package.npmDeps.packageLock);
  }
')

schema=$(jq -er '.lock | fromjson | .packages["node_modules/@earendil-works/pi-schema"].resolved' <<<"$metadata")
ai=$(jq -er '.lock | fromjson | .packages["node_modules/@earendil-works/pi-ai"].resolved' <<<"$metadata")
if [[ "$schema" != "${ai%/ai.tgz}/schema.tgz" || "$schema" != file:/nix/store/* ]]; then
  echo "FAIL: pi-schema must use the same workspace build as pi-ai: $schema" >&2
  exit 1
fi

workspace_drv=$(jq -er '.contexts | map(select(contains("-pi-workspace-packages-"))) | select(length == 1) | .[0]' <<<"$metadata")
# Query bindings directly; `nix derivation show` has version-dependent JSON envelopes.
phases=$(
  nix-store --query --binding installPhase "$workspace_drv" &&
    nix-store --query --binding postInstall "$workspace_drv"
)
if ! rg -q 'pack_package packages/schema schema' <<<"$phases"; then
  echo 'FAIL: workspace build does not pack schema.tgz' >&2
  exit 1
fi

echo "PASS: $system evaluates pi-schema from the workspace build and declares schema.tgz packing"
