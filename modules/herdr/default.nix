{
  config,
  lib,
  pkgs,
  themeExports,
  ...
}:

let
  cfg = config.modules.herdr;
in
{
  options.modules.herdr.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = "Enable Herdr module";
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ pkgs.herdr ];

    xdg.configFile."herdr/config.toml".text =
      builtins.readFile ./files/config.toml
      + "\n"
      + builtins.readFile (themeExports.file "herdr" "${config.modules.commonTheme}.toml");
  };
}
