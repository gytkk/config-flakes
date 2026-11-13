{
  config,
  lib,
  pkgs,
  username,
  flakeDirectory,
  isWSL ? false,
  ...
}:

let
  cfg = config.modules.herdrApp;
  appConfigHome =
    if isWSL then
      "/mnt/c/Users/${username}/.config/herdr-app"
    else
      "${config.xdg.configHome}/herdr-app";
in
{
  options.modules.herdrApp.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = "Manage Herdr App settings, shortcuts, prefix mode, and space completion indicators";
  };

  config = lib.mkIf cfg.enable {
    xdg.configFile."herdr-app/settings.json" = lib.mkIf (!isWSL) {
      source = config.lib.file.mkOutOfStoreSymlink "${flakeDirectory}/modules/herdr-app/files/settings.json";
    };

    home.activation.herdrAppKeys = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
      run ${pkgs.python3}/bin/python3 ${./install.py} ${./files} \
        ${lib.escapeShellArg appConfigHome} ${lib.optionalString isWSL "--copy-settings"}
    '';
  };
}
