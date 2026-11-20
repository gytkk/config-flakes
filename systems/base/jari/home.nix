{
  config,
  lib,
  ...
}:

let
  # jari pins its agent CLIs and wraps gh in these directories; they must win on PATH everywhere.
  jariPath = [
    "/opt/jari/bin"
    "/opt/jari-cli/node_modules/.bin"
    "/opt/node/bin"
  ];

  pathEntries = jariPath ++ [
    "${config.home.profileDirectory}/bin"
    "/run/wrappers/bin"
    "/run/current-system/sw/bin"
  ];
  pathHead = lib.concatStringsSep ":" pathEntries;

  # Shared by bash and zsh. Agent shell snapshots source ~/.bashrc non-interactively,
  # so this must stay silent, builtin-only, and unable to fail.
  normalizePath = ''
    __jari_normalize_path() {
      local rest result entry
      rest="${pathHead}:''${PATH:-}"
      result=""
      while [ -n "$rest" ]; do
        entry="''${rest%%:*}"
        case "$rest" in
          *:*) rest="''${rest#*:}" ;;
          *) rest="" ;;
        esac
        if [ -n "$entry" ]; then
          case ":$result:" in
            *":$entry:"*) ;;
            *) result="''${result:+$result:}$entry" ;;
          esac
        fi
      done
      PATH="$result"
      export PATH
    }
    __jari_normalize_path
    unset -f __jari_normalize_path
  '';

  zsh = "${config.programs.zsh.package}/bin/zsh";

in
{
  # Import base configuration
  imports = [
    ../default.nix
    ../devsisters/tools.nix
  ];

  modules = {
    # jari owns the agent CLIs and keeps rewriting their settings under ~/.claude and ~/.codex.
    agentSessionRecord.enable = lib.mkForce false;
    claude.enable = lib.mkForce false;
    codex.enable = lib.mkForce false;
    herdr.enable = lib.mkForce false;
    herdrApp.enable = lib.mkForce false;

    # jari leaves ~/.pi alone but runs its own pinned Pi from jari-worker.service's fixed
    # environment, which never reads the shell profile.
    pi = {
      settingsFile = ../../../modules/pi/files/settings-devsisters.json;
      installPackage = false;
      launchEnvironment = {
        enable = true;
        pathHead = pathEntries;
      };
    };

    # Every jari tab runs under jari's own tmux server, which reads the default
    # tmux config and must match the system tmux client.
    tmux.enable = lib.mkForce false;

    # Headless worker
    ghostty.enable = lib.mkForce false;
    windowsTerminal.enable = lib.mkForce false;
    zed.enable = lib.mkForce false;

    # A worker has no age identity, so the key cannot be decrypted.
    openaiApi.enable = lib.mkForce false;

    # The jari user has no systemd user session unless lingering is enabled.
    nixGc.enable = lib.mkForce false;
  };

  # Workers only ship the C.utf8 and en_US.utf8 locales.
  home.language.base = lib.mkForce "C.UTF-8";

  programs.git.settings = {
    # /etc/gitconfig carries jari's GitHub App credential helper; do not reset or replace it.
    credential = lib.mkForce { };

    # A worker has no SSH key. Fetch GitHub SSH remotes, including the private
    # git+ssh flake inputs, over HTTPS with jari's credential helper.
    url."https://github.com/".insteadOf = [
      "ssh://git@github.com/"
      "git@github.com:"
    ];
  };

  # jari starts shell tabs as `/bin/bash -l`, and agents read ~/.bashrc.
  programs.bash = {
    enable = true;

    # Runs before Home Manager's interactive guard, whose bare `return` reports
    # failure to a non-interactive `source ~/.bashrc`.
    bashrcExtra = normalizePath + ''
      [[ $- == *i* ]] || return 0
    '';

    initExtra = lib.mkOrder 50 ''
      # Hand interactive login shells to zsh. NO_ZSH_HANDOFF=1 keeps bash;
      # ZSH_HANDOFF_DONE lets a `bash -l` typed inside zsh stay in bash.
      if shopt -q login_shell \
        && [[ -z "''${NO_ZSH_HANDOFF:-}" && -z "''${ZSH_HANDOFF_DONE:-}" ]] \
        && [[ -z "''${BASH_EXECUTION_STRING:-}" && -t 0 && -t 1 && -x ${zsh} ]]; then
        export ZSH_HANDOFF_DONE=1
        exec ${zsh} -l
      fi
    '';
  };

  programs.zsh = {
    sessionVariables = {
      LANG = lib.mkForce "C.UTF-8";
      LC_CTYPE = lib.mkForce "C.UTF-8";
    };

    # hm-session-vars and nix-daemon.sh prepend the Nix profile, so reorder after
    # each place that can run them.
    envExtra = normalizePath;
    profileExtra = normalizePath;
    initContent = lib.mkOrder 1500 normalizePath;
  };
}
