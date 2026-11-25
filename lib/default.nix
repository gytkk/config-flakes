{ inputs, nixpkgs }:
let
  builders = import ./builders.nix { inherit inputs nixpkgs; };
in
{
  inherit builders;
  environments = import ./environments.nix {
    inherit (nixpkgs) lib;
  };
}
