{
  config,
  lib,
  pkgs,
  flakeDirectory,
  ...
}:

let
  cfg = config.modules.omp;
  mkSymlink =
    path: config.lib.file.mkOutOfStoreSymlink "${flakeDirectory}/config/modules/omp/${path}";
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
  };
}
