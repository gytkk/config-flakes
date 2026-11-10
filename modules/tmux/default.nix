{
  config,
  flakeDirectory,
  lib,
  pkgs,
  themeExports,
  ...
}:

let
  cfg = config.modules.tmux;
  mkSymlink = path: config.lib.file.mkOutOfStoreSymlink "${flakeDirectory}/modules/tmux/${path}";
  generatedThemes = themeExports.mutableDirLink config.lib.file "tmux";
  selectedTheme =
    themeExports.mutableFileLink config.lib.file "tmux"
      "${config.modules.commonTheme}.conf";
  tmuxSessionManager = pkgs.writeShellScriptBin "tm" ''
    if [ "$#" -eq 0 ] && [ -t 0 ] && [ -t 1 ] && [ -z "''${TMUX:-}" ]; then
      exec ${pkgs.bash}/bin/bash ${flakeDirectory}/modules/tmux/files/tmux-session-manager.sh ${pkgs.tmux}/bin/tmux ${pkgs.fzf}/bin/fzf
    fi

    exec ${pkgs.tmux}/bin/tmux "$@"
  '';
  tmuxAgentStatus = pkgs.writeShellScriptBin "tmux-agent-status" ''
    export PATH=${pkgs.coreutils}/bin:${pkgs.tmux}/bin:$PATH
    exec ${pkgs.bash}/bin/bash ${flakeDirectory}/modules/tmux/files/agent-status-set.sh "$@"
  '';
  tmuxAgentWindowStatus = pkgs.writeShellScriptBin "tmux-agent-window-status" ''
    export PATH=${pkgs.coreutils}/bin:$PATH
    exec ${pkgs.bash}/bin/bash ${flakeDirectory}/modules/tmux/files/agent-status-window.sh "$@"
  '';
  tmuxAgentRun = pkgs.writeShellScriptBin "agent-run" ''
    export PATH=${pkgs.coreutils}/bin:${pkgs.tmux}/bin:$PATH
    export TMUX_AGENT_STATUS_BIN=${tmuxAgentStatus}/bin/tmux-agent-status
    exec ${pkgs.bash}/bin/bash ${flakeDirectory}/modules/tmux/files/agent-run.sh "$@"
  '';
in
{
  options.modules.tmux.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = "Enable tmux module";
  };

  config = lib.mkIf cfg.enable {
    home.packages = [
      pkgs.tmux
      tmuxSessionManager
      tmuxAgentStatus
      tmuxAgentWindowStatus
      tmuxAgentRun
    ];

    xdg.configFile = {
      "tmux/tmux.conf".source = mkSymlink "files/tmux.conf";
      "tmux/keybindings.conf".source = mkSymlink "files/keybindings.conf";
      "tmux/themes".source = generatedThemes;
      "tmux/theme.conf".source = selectedTheme;
    };
  };
}
