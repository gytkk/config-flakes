#!/usr/bin/env bash
# Prepare the profile import. Only the worker owner edits the live NixOS configuration.
set -euo pipefail

checkout=/workspace/repos/gytkk/config-flakes
if [[ ! -e /etc/NIXOS || ! -e /etc/nixos/jari/default.nix || $(id -un) != jari ]]; then
  echo 'Run as jari on a NixOS worker after jari has installed its worker module.' >&2
  exit 1
fi
changes=$(git -C "$checkout" status --porcelain)
if [[ -n $changes ]]; then
  echo "Review and commit changes in $checkout before installing system configuration." >&2
  exit 1
fi

# Fetch private inputs as jari. Keep the reviewed sources in the import's store closure
# so unattended root rebuilds need neither the mutable checkout nor its Git credentials.
export GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=url.https://github.com/.insteadOf GIT_CONFIG_VALUE_0=ssh://git@github.com/
revision=$(git -C "$checkout" rev-parse HEAD)
module=$(nix eval --impure --raw --expr "let flake = builtins.getFlake \"git+file://$checkout?rev=$revision\"; in import (flake.outPath + \"/systems/base/jari/worker-import.nix\") flake")
sudo nix-store --realise "$module" --add-root /etc/nixos/devsisters-jari.nix >/dev/null
printf '%s\n' 'Keep ./image.nix and ./jari in /etc/nixos/configuration.nix and add ./devsisters-jari.nix to imports.' \
  'Then run: sudo nixos-rebuild switch'
