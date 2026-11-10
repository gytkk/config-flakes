{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.modules.nixGc;

  prune = import ../nix-gc/prune.nix {
    inherit (cfg) keepGenerations maxAge;
    inherit pkgs;
    nixPackage = config.nix.package;
    profilesDir = "/nix/var/nix/profiles";
  };
in
{
  options.modules.nixGc = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
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

    dates = lib.mkOption {
      type = lib.types.singleLineStr;
      default = "*-*-* 03:15:00";
      description = "systemd.time(7) calendar expression for when the collector runs.";
    };
  };

  config = lib.mkIf cfg.enable {
    # Superseded by the units below, which also cap the generation count.
    nix.gc.automatic = false;

    systemd.services.nix-gc-prune = {
      description = "Prune Nix generations and collect garbage";
      serviceConfig.Type = "oneshot";
      script = "exec ${prune}/bin/nix-gc-prune";
      startAt = cfg.dates;
    };

    systemd.timers.nix-gc-prune.timerConfig = {
      Persistent = true;
      RandomizedDelaySec = "1h";
    };

    # The collector deletes system generations that the bootloader would
    # otherwise still offer, so keep the boot menu to the same retention.
    boot.loader.grub.configurationLimit = lib.mkDefault cfg.keepGenerations;
    boot.loader.systemd-boot.configurationLimit = lib.mkDefault cfg.keepGenerations;
  };
}
