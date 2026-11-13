{ inputs }:
let
  lib = inputs.nixpkgs.lib;
  launchdSource = import ./launchd/backport.nix {
    inherit lib;
    source = builtins.readFile "${inputs.home-manager}/modules/launchd/default.nix";
  };
  launchdModule = builtins.toFile "home-manager-launchd.nix" (
    if builtins.length (lib.splitString "import ./launchd.nix" launchdSource) != 2 then
      throw "Home Manager launchd option import changed: review the compatibility module"
    else
      builtins.replaceStrings
        [ "import ./launchd.nix" ]
        [ "import ${inputs.home-manager}/modules/launchd/launchd.nix" ]
        launchdSource
  );
in
[
  (import ./agenix { inherit inputs; })
  {
    disabledModules = [ "${inputs.home-manager}/modules/launchd/default.nix" ];
    imports = [ launchdModule ];
  }
]
