{ inputs, systemPkgs }:
let
  mkFlakeDirectory = homeDirectory: "${homeDirectory}/development/nix-flakes";
in
{
  inherit mkFlakeDirectory;

  mkHomeConfig =
    { baseModules }:
    name: config:
    let
      requiredFields = [
        "system"
        "username"
        "homeDirectory"
        "profile"
      ];
      missingFields = builtins.filter (field: !(builtins.hasAttr field config)) requiredFields;
      pkgs = systemPkgs.${config.system};

      flakeDirectory = config.flakeDirectory or (mkFlakeDirectory config.homeDirectory);

      baseHomeModule = ../base + "/${config.profile}/home.nix";
      dynamicModules =
        if builtins.pathExists baseHomeModule then
          [ baseHomeModule ]
        else
          throw "Base profile '${config.profile}' not found at ${baseHomeModule}";
    in
    if missingFields != [ ] then
      throw "Missing required fields for ${name}: ${builtins.toString missingFields}"
    else
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
      };
}
