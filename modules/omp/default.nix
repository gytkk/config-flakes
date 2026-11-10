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
in
{
  options.modules.omp.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = "Enable OMP coding agent CLI and checkout-backed settings";
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ pkgs.omp ];
    home.file.".omp/agent/config.yml".source = mkSymlink "files/config.yml";
    home.file.".omp/agent/extensions/herdr-omp-agent-state.ts" = lib.mkIf config.modules.herdr.enable {
      source = "${pkgs.herdr.src}/src/integration/assets/omp/herdr-agent-state.ts";
    };
    home.file.".omp/agent/extensions/herdr-subagents.ts" = lib.mkIf config.modules.herdr.enable {
      source = mkSymlink "files/extensions/herdr-subagents.ts";
    };
  };
}
