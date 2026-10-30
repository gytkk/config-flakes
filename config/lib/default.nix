{ inputs, nixpkgs }:
let
  builders = import ./builders.nix { inherit inputs nixpkgs; };
in
{
  inherit builders;
  environments = import ./environments.nix {
    pkgs = nixpkgs;
    lib = nixpkgs.lib;
  };
}
