{
  config,
  lib,
  pkgs,
  flakeDirectory,
  ...
}:

let
  cfg = config.modules.aerospace;
  mkSymlink = path: config.lib.file.mkOutOfStoreSymlink "${flakeDirectory}/modules/aerospace/${path}";
  xdgConfigPath = "${config.xdg.configHome}/aerospace/aerospace.toml";
  legacyConfigPath = "${config.home.homeDirectory}/.aerospace.toml";
  legacyBackupPath = "${config.home.homeDirectory}/.aerospace.toml.pre-xdg-backup";
in
{
  options.modules.aerospace.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = "Enable AeroSpace module";
  };

  config = lib.mkIf (cfg.enable && pkgs.stdenv.hostPlatform.isDarwin) {
    xdg.configFile."aerospace/aerospace.toml".source = mkSymlink "files/aerospace.toml";

    home.activation.aerospaceLegacyConfig = lib.hm.dag.entryAfter [ "linkGeneration" ] ''
      migrate_aerospace_config() (
        managed_config="${xdgConfigPath}"
        legacy_config="${legacyConfigPath}"
        legacy_backup="${legacyBackupPath}"

        if [ ! -e "$managed_config" ] && [ ! -L "$managed_config" ]; then
          echo "Expected AeroSpace config at $managed_config, but it was not created." >&2
          exit 1
        fi

        if [ -e "$legacy_config" ] || [ -L "$legacy_config" ]; then
          if cmp -s "$legacy_config" "$managed_config"; then
            rm -f "$legacy_config"
          elif [ ! -e "$legacy_backup" ] && [ ! -L "$legacy_backup" ]; then
            mv "$legacy_config" "$legacy_backup"
          elif cmp -s "$legacy_config" "$legacy_backup"; then
            rm -f "$legacy_config"
          else
            echo "Legacy AeroSpace config at $legacy_config differs from the managed XDG config and the backup at $legacy_backup." >&2
            echo "Merge or remove it manually, then rerun home-manager switch." >&2
            exit 1
          fi
        fi
      )
      run migrate_aerospace_config
    '';
  };
}
