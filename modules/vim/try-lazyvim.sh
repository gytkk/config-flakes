#!/usr/bin/env bash
set -euo pipefail

module_dir=$(dirname "$(realpath "${BASH_SOURCE[0]}")")
config_home=${XDG_CONFIG_HOME:-$HOME/.config}
candidate_config="$config_home/nvim-lazyvim"
mkdir -p "$candidate_config"
if [[ ! -e "$candidate_config/themes" && ! -L "$candidate_config/themes" ]]; then
  ln -s "$config_home/nvim/themes" "$candidate_config/themes"
fi

export NVIM_APPNAME=nvim-lazyvim
export NVIM_LEGACY_CONFIG="$config_home/nvim"
exec "${NVIM_BIN:-nvim}" -u "$module_dir/files/lazyvim/init.lua" "$@"
