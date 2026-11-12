#!/usr/bin/env bash
# Install the devsisters-jari Home Manager environment on a jari worker.
#
# A worker keeps its disk across sleep and wake, but a new worker starts empty,
# so this script is safe to run again at any time. It needs no SSH key, age
# identity, or private flake input.
set -euo pipefail

repoUrl="https://github.com/gytkk/config-flakes"
# Must match flakeDirectory of devsisters-jari in systems/inventory.nix:
# out-of-store symlinks (Neovim Lua, themes) point into this checkout.
checkout="/workspace/repos/gytkk/config-flakes"
environment="devsisters-jari"
nixBin="/nix/var/nix/profiles/default/bin"

main() {
  if [ "$(id -un)" != "jari" ]; then
    echo "bootstrap: run as the jari user (current: $(id -un))" >&2
    exit 1
  fi

  if [ ! -e /nix ]; then
    echo "bootstrap: installing Nix (multi-user daemon)"
    curl --proto '=https' --tlsv1.2 -fsSL https://nixos.org/nix/install | sh -s -- --daemon --yes
  fi

  # Neither agent shells nor a just-installed Nix have it on PATH, and the
  # worker's /etc/nix/nix.conf does not enable flakes. Activation checks USER.
  export USER="${USER:-jari}"
  export PATH="$PATH:$nixBin"
  export NIX_CONFIG="${NIX_CONFIG:+$NIX_CONFIG
}extra-experimental-features = nix-command flakes"

  if [ -e "$checkout/.git" ]; then
    echo "bootstrap: using existing checkout $checkout ($(git -C "$checkout" rev-parse --abbrev-ref HEAD))"
  else
    echo "bootstrap: cloning $repoUrl to $checkout"
    mkdir -p "$(dirname "$checkout")"
    git clone "$repoUrl" "$checkout"
  fi

  echo "bootstrap: building $environment"
  generation="$(
    nix build "$checkout#homeConfigurations.$environment.activationPackage" \
      --extra-experimental-features 'nix-command flakes' \
      --no-link --print-out-paths
  )"

  # The stock Ubuntu ~/.bashrc and ~/.profile are moved aside instead of aborting the switch.
  echo "bootstrap: activating $generation"
  HOME_MANAGER_BACKUP_EXT="${HOME_MANAGER_BACKUP_EXT:-pre-home-manager}" "$generation/activate"

  echo "bootstrap: done. Open a new shell tab and start new agent sessions to pick up the environment."
}

main "$@"
