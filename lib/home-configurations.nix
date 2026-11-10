{ inputs, systemPkgs }:
let
  mkFlakeDirectory = homeDirectory: "${homeDirectory}/ws";
in
{
  inherit mkFlakeDirectory;

  mkHomeConfig =
    name: config:
    let
      validateInventory = import ./inventory-validation.nix;
      pkgs = systemPkgs.${config.system};

      flakeDirectory = config.flakeDirectory or (mkFlakeDirectory config.homeDirectory);

      baseHomeModule = ../systems/base + "/${config.profile}/home.nix";
      dynamicModules =
        if builtins.pathExists baseHomeModule then
          [ baseHomeModule ]
        else
          throw "Base profile '${config.profile}' not found at ${baseHomeModule}";
    in
    validateInventory name config (
      inputs.home-manager.lib.homeManagerConfiguration {
        inherit pkgs;
        extraSpecialArgs = {
          inherit (config) username homeDirectory;
          inherit inputs flakeDirectory;
          themeExports = import ./themes.nix { inherit flakeDirectory; };
          isWSL = config.isWSL or false;
          hasSystemCodexConfig = false;
          hasSystemNiriConfig = false;
        };
        modules =
          (import ./home-manager-modules.nix { inherit inputs; })
          ++ dynamicModules
          ++ (config.homeModules or [ ]);
      }
    );
}
