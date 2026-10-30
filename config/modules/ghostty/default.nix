{
  config,
  lib,
  themeExports,
  ...
}:

let
  cfg = config.modules.ghostty;
  ghosttyConfig = builtins.replaceStrings [ "__COMMON_THEME__" ] [ "${config.modules.commonTheme}" ] (
    builtins.readFile ./files/config
  );
in
{
  options.modules.ghostty.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = "Enable Ghostty module";
  };

  config = lib.mkIf cfg.enable {
    xdg.configFile."ghostty/themes".source = themeExports.dir "ghostty";
    xdg.configFile."ghostty/config".text = ghosttyConfig;
  };
}
