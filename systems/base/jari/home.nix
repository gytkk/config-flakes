{
  config,
  lib,
  pkgs,
  ...
}:

let
  # jari pins its agent CLIs and wraps gh in these directories; they must win on PATH everywhere.
  jariPath = [
    "/opt/jari/bin"
    "/opt/jari-cli/node_modules/.bin"
    "/opt/node/bin"
  ];

  pathHead = lib.concatStringsSep ":" (
    jariPath
    ++ [
      "${config.home.profileDirectory}/bin"
      "/nix/var/nix/profiles/default/bin"
    ]
  );

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
  imports = [ ../default.nix ];

  modules = {
    # jari owns the agent CLIs and keeps rewriting their settings under ~/.claude and ~/.codex.
    agentSessionRecord.enable = lib.mkForce false;
    claude.enable = lib.mkForce false;
    codex.enable = lib.mkForce false;
    herdr.enable = lib.mkForce false;
    herdrApp.enable = lib.mkForce false;
    pi.enable = lib.mkForce false;

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

  # Devsisters tools that need neither private flake inputs nor secrets
  home.packages = with pkgs; [
    vault
    kubelogin-oidc
    databricks-cli
    pantry
  ];

  home.sessionVariables = {
    VAULT_ADDR = "https://vault.devsisters.cloud";
  };

  # Workers only ship the C.utf8 and en_US.utf8 locales.
  home.language.base = lib.mkForce "C.UTF-8";

  programs.git.settings = {
    user = {
      email = lib.mkForce "gyutak.kim@devsisters.com";
      name = lib.mkForce "Gyutak Kim";
    };

    # /etc/gitconfig carries jari's GitHub App credential helper; do not reset or replace it.
    credential = lib.mkForce { };
  };

  # The worker's /etc/nix/nix.conf does not enable flakes.
  nix = {
    package = pkgs.nix;
    settings.experimental-features = [
      "nix-command"
      "flakes"
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
