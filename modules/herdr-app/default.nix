{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.modules.herdrApp;
in
{
  options.modules.herdrApp.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = "Manage Herdr App shortcuts, prefix mode, and OMP subagent display";
  };

  config = lib.mkIf cfg.enable {
    home.activation.herdrAppKeys = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
      run ${pkgs.python3}/bin/python3 ${./install.py} ${./files} \
        ${lib.escapeShellArg "${config.xdg.configHome}/herdr-app"}
    '';
  };
}
