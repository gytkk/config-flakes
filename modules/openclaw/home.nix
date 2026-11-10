{
  config,
  lib,
  pkgs,
  homeDirectory,
  ...
}:
let
  cfg = config.modules.openclaw;
  stateDir = toString cfg.stateDir;
  gatewaySystemdDropInPath = "${homeDirectory}/.config/systemd/user/openclaw-gateway.service.d/20-nix-runtime.conf";
  gatewaySystemdDropIn = pkgs.writeText "20-nix-runtime.conf" ''
    [Service]
    UnsetEnvironment=CLAWDBOT_CONFIG_PATH CLAWDBOT_STATE_DIR
    Environment="LD_LIBRARY_PATH=${lib.makeLibraryPath [ pkgs.libcap ]}"
  '';
in
{
  options.modules.openclaw = {
    enable = lib.mkEnableOption "runtime integration for a user-managed OpenClaw installation";

    stateDir = lib.mkOption {
      type = lib.types.str;
      default = "${homeDirectory}/.openclaw";
      description = "Mutable OpenClaw state and installation directory.";
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = lib.hasPrefix "/" stateDir;
        message = "modules.openclaw.stateDir must be an absolute path.";
      }
      {
        assertion = !(lib.hasPrefix "/nix/store/" stateDir);
        message = "modules.openclaw.stateDir must point to mutable host storage, not the Nix store.";
      }
    ];

    home.sessionPath = [ "${stateDir}/bin" ];

    # OpenClaw rejects symlinked effective service definitions during update
    # ownership checks, so keep this supported Environment drop-in materialized.
    home.activation.materializeOpenClawSystemdDropIn = lib.hm.dag.entryAfter [ "linkGeneration" ] ''
      dropInPath=${lib.escapeShellArg gatewaySystemdDropInPath}
      if [ -L "$dropInPath" ]; then
        run ${pkgs.coreutils}/bin/rm "$dropInPath"
      fi
      if [ ! -f "$dropInPath" ] || ! ${pkgs.coreutils}/bin/cmp -s ${gatewaySystemdDropIn} "$dropInPath"; then
        run ${pkgs.coreutils}/bin/install -D -m 0644 ${gatewaySystemdDropIn} "$dropInPath"
      fi
    '';

  };
}
