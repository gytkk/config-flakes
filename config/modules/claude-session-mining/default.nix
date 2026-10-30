{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.modules.claudeSessionMining;
  logDirectory = "${config.home.homeDirectory}/Library/Logs/claude-mining";
in
{
  options.modules.claudeSessionMining = {
    enable = lib.mkEnableOption "daily Claude session mining launchd agent";

    runnerPath = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      description = "Absolute path to the externally managed session-mining runner.";
    };
  };

  config = lib.mkMerge [
    {
      assertions = [
        {
          assertion = !cfg.enable || pkgs.stdenv.hostPlatform.isDarwin;
          message = "modules.claudeSessionMining only supports Darwin hosts.";
        }
        {
          assertion = !cfg.enable || (cfg.runnerPath != null && lib.hasPrefix "/" cfg.runnerPath);
          message = "modules.claudeSessionMining.runnerPath must be an absolute path when enabled.";
        }
      ];
    }

    (lib.mkIf (cfg.enable && pkgs.stdenv.hostPlatform.isDarwin) {
      launchd.agents.claude-session-mining = {
        enable = true;
        config = {
          ProgramArguments = [ cfg.runnerPath ];
          StartCalendarInterval = [
            {
              Hour = 6;
              Minute = 0;
            }
          ];
          ProcessType = "Background";
          StandardOutPath = "${logDirectory}/launchd.stdout";
          StandardErrorPath = "${logDirectory}/launchd.stderr";
        };
      };

      home.activation.claudeMiningLogDir = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
        run mkdir -p ${lib.escapeShellArg logDirectory}
        if [ ! -x ${lib.escapeShellArg cfg.runnerPath} ]; then
          echo ${lib.escapeShellArg "claude-session-mining: runner is missing or not executable: ${cfg.runnerPath}"} >&2
        fi
      '';
    })
  ];
}
