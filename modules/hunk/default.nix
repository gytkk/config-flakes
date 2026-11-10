{
  config,
  lib,
  ...
}:

let
  cfg = config.modules.hunk;
in
{
  options.modules.hunk.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = "Enable Hunk configuration";
  };

  config = lib.mkIf cfg.enable {
    xdg.configFile."hunk/config.toml".source = ./files/config.toml;
  };
}
