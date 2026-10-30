{
  config,
  lib,
  pkgs,
  ...
}:
let
  stateDirectory = "${config.xdg.stateHome}/agenix-launchd";
  wrapperPath = "${config.xdg.stateHome}/agenix-launchd-wrapper";
  runner = "${pkgs.python3}/bin/python3 ${./runner.py}";
  wrapperSource = pkgs.writeScript "agenix-launchd-wrapper" ''
    #!/bin/sh
    exec ${runner} login --state-dir ${lib.escapeShellArg stateDirectory}
  '';
in
{
  config = lib.mkIf pkgs.stdenv.hostPlatform.isDarwin (
    lib.mkMerge [
      {
        # Long-lived consumers must not lose secrets when macOS clears temporary files.
        age.secretsDir = "${config.xdg.stateHome}/agenix";
        age.secretsMountPoint = "${config.xdg.stateHome}/agenix.d";
      }
      (lib.mkIf (config.age.secrets != { }) {
        launchd.agents.activate-agenix.config = {
          KeepAlive = lib.mkForce null;
          ProgramArguments = lib.mkForce [ wrapperPath ];
        };

        home.activation.writeAgenixLaunchdWrapper =
          lib.hm.dag.entryBetween [ "setupLaunchAgents" ] [ "writeBoundary" ]
            ''
              run mkdir -p ${lib.escapeShellArg "${config.home.homeDirectory}/Library/Logs/agenix"}
              run ${runner} activate \
                --state-dir ${lib.escapeShellArg stateDirectory} \
                --mount-script ${lib.escapeShellArg config.age.mountingScript} \
                --wrapper-source ${wrapperSource} \
                --wrapper-path ${lib.escapeShellArg wrapperPath} \
                --secrets-dir ${lib.escapeShellArg config.age.secretsDir} \
                || warnEcho "[agenix] secret mount failed; retry by applying Home Manager again or logging in (see ~/Library/Logs/agenix)"
            '';
      })
    ]
  );
}
