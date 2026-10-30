{
  description = "Home Manager and NixOS configuration";

  inputs.config.url = "path:./config";

  outputs =
    { config, ... }:
    {
      inherit (config)
        homeConfigurations
        nixosConfigurations
        packages
        apps
        checks
        ;
    };
}
