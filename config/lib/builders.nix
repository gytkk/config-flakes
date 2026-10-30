{ inputs, nixpkgs }:
let
  pkgsLib = import ./pkgs.nix { inherit inputs nixpkgs; };
  homeLib = import ./home-configurations.nix {
    inherit inputs;
    systemPkgs = pkgsLib.systemPkgs;
  };
  nixosLib = import ./nixos-configurations.nix {
    inherit inputs nixpkgs;
    inherit (pkgsLib) commonOverlays;
    inherit (homeLib) mkFlakeDirectory;
  };
in
pkgsLib // homeLib // nixosLib
