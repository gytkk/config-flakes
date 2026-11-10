{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.modules.terraform;
in
{
  options.modules.terraform = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Enable Terraform version management with tenv";
    };

    runEnv = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = { };
      description = "Environment variables to set when running terraform";
      example = {
        TF_VAR_ENVIRONMENT = "dev";
        AWS_REGION = "ap-northeast-1";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = with pkgs; [ tenv ];

    home.sessionVariables = {
      TENV_AUTO_INSTALL = "true";
      # Project version files take precedence over this constraint-aware fallback.
      TFENV_TERRAFORM_DEFAULT_VERSION = "latest-allowed";
    };

    home.shellAliases = {
      tf =
        let
          envPrefix = lib.optionalString (cfg.runEnv != { }) (
            lib.concatStringsSep " " (lib.mapAttrsToList (name: value: "${name}=${value}") cfg.runEnv) + " "
          );
        in
        "${envPrefix}terraform";
    };
  };
}
