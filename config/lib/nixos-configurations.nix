{
  inputs,
  nixpkgs,
  commonOverlays,
  mkFlakeDirectory,
}:
{
  mkNixOSConfig =
    name: config:
    let
      requiredFields = [
        "system"
        "username"
        "homeDirectory"
        "profile"
      ];
      missingFields = builtins.filter (field: !(builtins.hasAttr field config)) requiredFields;

      flakeDirectory = config.flakeDirectory or (mkFlakeDirectory config.homeDirectory);

      specialArgs = {
        inherit inputs flakeDirectory;
        inherit (config) username homeDirectory;
        themeExports = import ./themes.nix { inherit flakeDirectory; };
        isWSL = config.isWSL or false;
        hasSystemCodexConfig = true;
        hasSystemNiriConfig = true;
      };

      homeConfig = ../base + "/${config.profile}/home.nix";
      homeModules = [ homeConfig ] ++ (config.homeModules or [ ]);
      sharedHomeModules = import ./home-manager-modules.nix { inherit inputs; };
    in
    if missingFields != [ ] then
      throw "Missing required fields for NixOS host ${name}: ${builtins.toString missingFields}"
    else
      nixpkgs.lib.nixosSystem {
        inherit specialArgs;
        modules = [
          inputs.home-manager.nixosModules.home-manager
          (../hosts + "/${name}/configuration.nix")
          {
            nixpkgs.hostPlatform = config.system;
            nixpkgs.overlays = commonOverlays;
            nixpkgs.config.allowUnfree = true;

            home-manager.useGlobalPkgs = true;
            home-manager.useUserPackages = true;
            home-manager.backupFileExtension = "backup";
            home-manager.extraSpecialArgs = specialArgs;
            home-manager.sharedModules = sharedHomeModules;
            home-manager.users.${config.username} = {
              imports = homeModules;
            };
          }
        ]
        ++ (config.extraModules or [ ]);
      };
}
