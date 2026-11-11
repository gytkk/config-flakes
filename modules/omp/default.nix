{
  config,
  lib,
  pkgs,
  flakeDirectory,
  ...
}:

let
  cfg = config.modules.omp;
  mkSymlink = path: config.lib.file.mkOutOfStoreSymlink "${flakeDirectory}/modules/omp/${path}";
  fileLinks =
    config.modules.herdrApp.enable
    && config.programs.neovim.enable
    && pkgs.stdenv.hostPlatform.isDarwin;
  package =
    if fileLinks then
      pkgs.symlinkJoin {
        name = "omp-herdr";
        paths = [ pkgs.omp ];
        nativeBuildInputs = [ pkgs.makeWrapper ];
        postBuild = ''
          rm "$out/bin/omp"
          makeWrapper ${pkgs.python3}/bin/python3 "$out/bin/omp" \
            --add-flags "${../herdr-app/file-opener.py} launch ${pkgs.omp}/bin/omp"
        '';
      }
    else
      pkgs.omp;
in
{
  options.modules.omp.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = "Enable OMP coding agent CLI and checkout-backed settings";
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ package ];
    home.file.".omp/agent/config.yml".source = mkSymlink "files/config.yml";
    home.file.".omp/agent/extensions/herdr-omp-agent-state.ts" = lib.mkIf config.modules.herdr.enable {
      source = "${pkgs.herdr.src}/src/integration/assets/omp/herdr-agent-state.ts";
    };
    home.file.".omp/agent/extensions/herdr-subagents.ts" =
      lib.mkIf (config.modules.herdr.enable || config.modules.herdrApp.enable)
        {
          source = mkSymlink "files/extensions/herdr-subagents.ts";
        };
  };
}
