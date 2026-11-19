#!/usr/bin/env bash
set -euo pipefail

root=$(git rev-parse --show-toplevel)/modules/pi
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
export HOME="$tmp/home" PI_CODING_AGENT_DIR="$tmp/home/.pi/agent"
file="$PI_CODING_AGENT_DIR/mcp.json"
cp "$root/files/mcp.json" "$tmp/legacy"
seed() { bash "$root/files/seed-mcp.sh" "$tmp/legacy"; }

seed
jq -e '(.mcpServers | keys) == ["context7", "nixos"]' "$file" >/dev/null
pi mcp add nixos -- custom-nixos
pi mcp add personal -- personal-command
pi mcp remove context7
seed
jq -e '.mcpServers | .nixos.command == "custom-nixos" and .personal.command == "personal-command" and .context7.url == "https://mcp.context7.com/mcp"' "$file" >/dev/null
cp "$file" "$tmp/before"
seed
cmp "$tmp/before" "$file"

rm "$file"
ln -s "$tmp/legacy" "$file"
seed
[[ ! -L $file ]]
cmp "$file" "$tmp/legacy"
pi mcp remove plane
cmp "$tmp/legacy" "$root/files/mcp.json"
seed
jq -e '.mcpServers | has("plane") | not' "$file" >/dev/null

printf 'invalid\n' > "$file"
if seed; then exit 1; fi
[[ $(<"$file") == invalid ]]
