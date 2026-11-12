{ inputs, nixpkgs }:
let
  nixpkgsVersionsOverlay =
    final: _prev:
    let
      mkLazyPkgs =
        nixpkgsInput: packages:
        builtins.listToAttrs (
          map (name: {
            inherit name;
            value =
              (import nixpkgsInput {
                localSystem = final.stdenv.hostPlatform.system;
                config.allowUnfree = true;
              }).${name};
          }) packages
        );
    in
    {
      stable-25_05 = mkLazyPkgs inputs.nixpkgs-25_05 [ "ruby_3_2" ];
    }
    // import ./lsp.nix {
      nixpkgs = inputs.nixpkgs-lsp;
      system = final.stdenv.hostPlatform.system;
    };

  localAppsOverlay =
    final: prev:
    import ../../packages {
      pkgs = final;
      inherit (prev.stdenv.hostPlatform) isDarwin system;
    };

  commonOverlays = [
    inputs.copyparty.overlays.default
    inputs.nix-zed-extensions.overlays.default
    localAppsOverlay
    inputs.niri.overlays.niri
    nixpkgsVersionsOverlay
    (final: _prev: {
      agent-core = final.callPackage ../../agent-core/nix/package.nix { };
    })
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
    # Home Manager only (devsisters-jari); outputs.nix exposes no packages, apps, or checks for it.
    "aarch64-linux" = import nixpkgs {
      localSystem = "aarch64-linux";
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
