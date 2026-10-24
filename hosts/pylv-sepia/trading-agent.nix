{ ... }:
let
  productSource = import ./trading-agent-source.nix;
in
{
  imports = [ (productSource + "/nix/module.nix") ];

  services.trading-agent-admin = {
    enable = true;
    hostname = "trading.pylv.dev";
    uid = 1000;
    gid = 100;
    registryFile = "/home/gytkk/.config/trading-agent/profiles.toml";
    accessConfigFile = "/home/gytkk/.config/trading-agent/access.toml";
    databaseDirectory = "/home/gytkk/.local/share/trading-agent/imports/pylv-onyx-20261024/databases";
    accountsConfigFile = "/home/gytkk/.config/trading-agent/live-accounts/accounts.toml";
    accountConfigsDirectory = "/home/gytkk/.config/trading-agent/live-accounts/configs";
    accountCredentialsDirectory = "/home/gytkk/.config/trading-agent/live-accounts/credentials";
  };
}
