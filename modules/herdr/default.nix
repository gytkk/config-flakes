{
  config,
  flakeDirectory,
  lib,
  pkgs,
  themeExports,
  ...
}:

let
  cfg = config.modules.herdr;
  mkSymlink = path: config.lib.file.mkOutOfStoreSymlink "${flakeDirectory}/modules/herdr/${path}";
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
    xdg.configFile."plannotator-tui/config.toml".source = mkSymlink "files/plannotator-tui.toml";

    home.activation.linkHerdrAnnotate = lib.hm.dag.entryAfter [ "linkGeneration" ] ''
      run ${pkgs.herdr}/bin/herdr plugin link \
        ${pkgs.herdr-annotate}/share/herdr/plugins/annotate --enabled
    '';

    home.activation.linkHerdrAutoTitle = lib.hm.dag.entryAfter [ "linkGeneration" ] ''
      run ${pkgs.herdr}/bin/herdr plugin link \
        ${pkgs.herdr-auto-title}/share/herdr/plugins/auto-title --enabled
    '';
  };
}
