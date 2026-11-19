#!/usr/bin/env bash
set -euo pipefail

export PI_CODING_AGENT_DIR="$HOME/.pi/agent"
file="$PI_CODING_AGENT_DIR/mcp.json"
legacy=${1:?Expected legacy MCP source path}

if [[ -L $file && ! $file -ef $legacy ]]; then
  echo "Refusing to modify an unrecognized MCP symlink: $file" >&2
  exit 1
fi
if [[ -e $file ]]; then
  jq -se 'length == 1 and (.[0] | type == "object" and ((.mcpServers // {}) | type == "object"))' "$file" >/dev/null || {
    echo "Invalid MCP configuration; leaving $file unchanged" >&2
    exit 1
  }
fi
if [[ -L $file ]]; then
  tmp=$(mktemp "$file.XXXXXX")
  trap 'rm -f "$tmp"' EXIT
  cp -L "$file" "$tmp"
  chmod 600 "$tmp"
  mv -f "$tmp" "$file"
fi

add_missing() {
  if [[ -f $file ]]; then
    local present
    present=$(jq -r --arg name "$1" '(.mcpServers // {}) | has($name)' "$file")
    [[ $present != true ]] || return 0
  fi
  pi mcp add "$@"
}

add_missing nixos -- mcp-nixos
add_missing context7 --url https://mcp.context7.com/mcp
