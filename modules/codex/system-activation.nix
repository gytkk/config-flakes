{
  lib,
  coreutils,
  configSource,
}:

let
  systemCodexConfigDirectory = "/etc/codex";
  systemCodexConfigPath = "${systemCodexConfigDirectory}/config.toml";
  legacyManagedCodexConfigPath = "${systemCodexConfigDirectory}/managed_config.toml";
  mkSymlinkCheck = destination: target: ''
    if [ -L ${lib.escapeShellArg destination} ]; then
      current_target="$(${coreutils}/bin/readlink ${lib.escapeShellArg destination} || true)"
      if [ "$current_target" != ${lib.escapeShellArg target} ]; then
        needs_update=1
      fi
    elif [ -e ${lib.escapeShellArg destination} ]; then
      errorEcho "${destination} exists and is not a symlink. Move it aside and rerun home-manager switch."
      exit 1
    else
      needs_update=1
    fi
  '';
in
''
  ensure_system_symlink() {
    local destination="$1"
    local desired_target="$2"
    local current_target=""

    if [ -L "$destination" ]; then
      current_target="$(${coreutils}/bin/readlink "$destination" || true)"
    elif [ -e "$destination" ]; then
      errorEcho "$destination exists and is not a symlink. Move it aside and rerun home-manager switch."
      exit 1
    fi

    if [ "$current_target" = "$desired_target" ]; then
      return 0
    fi

    run "$sudo_bin" ${coreutils}/bin/rm -f "$destination"
    run "$sudo_bin" ${coreutils}/bin/ln -s "$desired_target" "$destination"

    if [ -n "''${DRY_RUN:-}" ]; then
      return 0
    fi

    current_target="$(${coreutils}/bin/readlink "$destination" || true)"
    if [ "$current_target" != "$desired_target" ]; then
      errorEcho "Expected $destination to point to $desired_target, got '$current_target'."
      exit 1
    fi
  }

  ensure_system_codex_config() {
    local config_target=${lib.escapeShellArg configSource}
    local sudo_bin=""
    local needs_update=0
    local current_target=""
    local legacy_managed_target=""

    ${mkSymlinkCheck systemCodexConfigPath configSource}

    if [ -L ${lib.escapeShellArg legacyManagedCodexConfigPath} ]; then
      legacy_managed_target="$(${coreutils}/bin/readlink ${lib.escapeShellArg legacyManagedCodexConfigPath} || true)"
      if [ -n "$legacy_managed_target" ]; then
        needs_update=1
      fi
    elif [ -e ${lib.escapeShellArg legacyManagedCodexConfigPath} ]; then
      errorEcho "${legacyManagedCodexConfigPath} exists and is not a symlink. Move it aside and rerun home-manager switch."
      exit 1
    fi

    if [ "$needs_update" != "1" ]; then
      return 0
    fi

    for candidate in /run/wrappers/bin/sudo /usr/bin/sudo /bin/sudo; do
      if [ -x "$candidate" ]; then
        sudo_bin="$candidate"
        break
      fi
    done

    if [ -z "$sudo_bin" ]; then
      errorEcho "Could not find sudo while managing /etc/codex."
      exit 1
    fi

    if ! "$sudo_bin" -n true 2>/dev/null && ! [ -t 0 ] && ! [ -t 1 ] && ! [ -t 2 ]; then
      errorEcho "Managing /etc/codex requires sudo, but no interactive terminal is available."
      errorEcho "Rerun home-manager switch from an interactive shell or refresh sudo credentials first."
      exit 1
    fi

    run "$sudo_bin" ${coreutils}/bin/mkdir -p ${lib.escapeShellArg systemCodexConfigDirectory}
    if [ -n "$legacy_managed_target" ]; then
      run "$sudo_bin" ${coreutils}/bin/rm -f ${lib.escapeShellArg legacyManagedCodexConfigPath}
    fi

    ensure_system_symlink ${lib.escapeShellArg systemCodexConfigPath} "$config_target"
  }

  ensure_system_codex_config
''
