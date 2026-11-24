{ inputs }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  inherit (inputs) nixpkgs;
  host = (import ../../inventory.nix).devsisters-jari;
  profilePkgs = (import ../../../lib/pkgs { inherit inputs nixpkgs; }).systemPkgs.${host.system};
in
{
  imports = [
    inputs.home-manager.nixosModules.home-manager
    inputs.agenix.nixosModules.default
  ];

  options.modules.jari.hindsightAgenix.enable =
    lib.mkEnableOption "Hindsight token provisioning with the enrolled worker's SSH host identity";

  config = {
    age = lib.mkIf config.modules.jari.hindsightAgenix.enable {
      identityPaths = [ "/etc/ssh/ssh_host_ed25519_key" ];
      secrets.hindsight-client-token = {
        file = ../../../secrets/hindsight-client-token.age;
        path = "/run/secrets/hindsight-client-token";
        owner = host.username;
        mode = "0400";
      };
    };

    assertions = [
      {
        assertion = pkgs.stdenv.hostPlatform.system == host.system;
        message = "The devsisters-jari profile requires ${host.system}.";
      }
    ];

    services.tailscale.enable = true;

    home-manager = {
      useUserPackages = true;
      backupFileExtension = "pre-home-manager";
      extraSpecialArgs = {
        inherit inputs;
        inherit (host) username homeDirectory flakeDirectory;
        themeExports = import ../../../lib/themes.nix { inherit (host) flakeDirectory; };
        # Home Manager master uses lib functions not present in the AMI's 26.05 channel.
        lib = import "${inputs.home-manager}/modules/lib/stdlib-extended.nix" nixpkgs.lib;
        pkgs = profilePkgs;
        isWSL = false;
      };
      sharedModules = import ../../../lib/home-manager-modules.nix { inherit inputs; };
      users.${host.username}.imports = [ ./home.nix ];
    };

    # jari-worker.service includes the system profile on PATH, but not the user's profile.
    environment.systemPackages = [
      profilePkgs.databricks-cli
      inputs.keycloak2aws.packages.${host.system}.kc2aws
      profilePkgs.vault
      profilePkgs.pantry
    ];
  };
}
