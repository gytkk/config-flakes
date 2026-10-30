{ inputs, nixpkgs }:
let
  repoOverlays = import ../overlays { inherit inputs; };
  localAppsOverlay =
    final: prev:
    import ../packages {
      pkgs = final;
      inherit (prev.stdenv.hostPlatform) isDarwin system;
    };

  commonOverlays = [
    inputs.copyparty.overlays.default
    inputs.nix-zed-extensions.overlays.default
    localAppsOverlay
    inputs.niri.overlays.niri
    repoOverlays.nixpkgs-versions
    repoOverlays.package-fixes
  ];

  systemPkgs = {
    "x86_64-linux" = import nixpkgs {
      localSystem = "x86_64-linux";
      config.allowUnfree = true;
      overlays = commonOverlays;
    };
    "aarch64-darwin" = import nixpkgs {
      localSystem = "aarch64-darwin";
      config.allowUnfree = true;
      overlays = commonOverlays;
    };
  };
in
{
  inherit commonOverlays systemPkgs;

  mkPkgs = system: systemPkgs.${system};

  mkSystemPkgs =
    systems:
    builtins.listToAttrs (
      map (system: {
        name = system;
        value = systemPkgs.${system};
      }) systems
    );
}
