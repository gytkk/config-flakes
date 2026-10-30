{
  config,
  lib,
  pkgs,
  homeDirectory,
  ...
}:

let
  cfg = config.modules.nixGc;

  nixPackage =
    if config.nix.enable && config.nix.package != null then config.nix.package else pkgs.nix;

  logDir = "${homeDirectory}/Library/Logs/nix-gc";

  prune = import ./prune.nix {
    inherit (cfg) keepGenerations maxAge;
    inherit pkgs nixPackage;
    profilesDir = ''"''${XDG_STATE_HOME:-$HOME/.local/state}/nix/profiles"'';
  };

  startTime = "${lib.fixedWidthNumber 2 cfg.hour}:${lib.fixedWidthNumber 2 cfg.minute}";
in
{
  options.modules.nixGc = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Enable scheduled Nix generation pruning and store garbage collection";
    };

    keepGenerations = lib.mkOption {
      type = lib.types.ints.positive;
      default = 5;
      description = "Number of most recent generations kept per profile.";
    };

    maxAge = lib.mkOption {
      type = lib.types.strMatching "[0-9]+d";
      default = "14d";
      example = "30d";
      description = "Generations created more than this long ago are removed.";
    };

    hour = lib.mkOption {
      type = lib.types.ints.between 0 23;
      default = 3;
      description = "Hour at which the collector runs.";
    };

    minute = lib.mkOption {
      type = lib.types.ints.between 0 59;
      default = 15;
      description = "Minute at which the collector runs.";
    };
  };

  config = lib.mkIf cfg.enable (
    lib.mkMerge [
      (lib.mkIf pkgs.stdenv.hostPlatform.isDarwin {
        launchd.agents.nix-gc = {
          # Home Manager's launchd.agents.<name>.enable defaults to false, so a
          # config-only definition silently produces no plist at all.
          enable = true;
          config = {
            ProgramArguments = [ "${prune}/bin/nix-gc-prune" ];
            StartCalendarInterval = [
              {
                Hour = cfg.hour;
                Minute = cfg.minute;
              }
            ];
            ProcessType = "Background";
            StandardOutPath = "${logDir}/launchd.stdout";
            StandardErrorPath = "${logDir}/launchd.stderr";
          };
        };
      })

      (lib.mkIf pkgs.stdenv.hostPlatform.isLinux {
        systemd.user.services.nix-gc = {
          Unit.Description = "Prune Nix generations and collect garbage";
          Service = {
            Type = "oneshot";
            ExecStart = "${prune}/bin/nix-gc-prune";
          };
        };

        systemd.user.timers.nix-gc = {
          Unit.Description = "Prune Nix generations and collect garbage";
          Timer = {
            OnCalendar = "*-*-* ${startTime}:00";
            Persistent = true;
            RandomizedDelaySec = "1h";
            Unit = "nix-gc.service";
          };
          Install.WantedBy = [ "timers.target" ];
        };
      })
    ]
  );
}
