{
  description = "Self-contained non-nixpkgs app packages for nix-flakes.";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs =
    { nixpkgs, ... }:
    let
      systems = [
        "aarch64-darwin"
        "x86_64-linux"
        "aarch64-linux"
      ];

      forEachSystem = f: nixpkgs.lib.genAttrs systems (system: f system);

      pkgsFor =
        system:
        import nixpkgs {
          inherit system;
          config.allowUnfree = true;
        };

      packagesFor = system: import ./. { pkgs = pkgsFor system; };

      appNamesFor = system: builtins.attrNames (packagesFor system);

      packageFor = system: name: (packagesFor system).${name};

      mkPackages =
        system:
        let
          appNames = appNamesFor system;
        in
        if appNames == [ ] then
          { }
        else
          let
            defaultName = builtins.head appNames;
          in
          builtins.listToAttrs (
            [
              {
                name = "default";
                value = packageFor system defaultName;
              }
            ]
            ++ map (name: {
              name = name;
              value = packageFor system name;
            }) appNames
          );

      mkChecks =
        system:
        let
          buildChecks = builtins.listToAttrs (
            map (name: {
              name = "${name}-build";
              value = packageFor system name;
            }) (appNamesFor system)
          );
        in
        buildChecks;

      mkApps =
        system:
        builtins.listToAttrs (
          map (
            name:
            let
              pkg = packageFor system name;
            in
            {
              name = name;
              value = {
                type = "app";
                meta = {
                  description = pkg.meta.description or "${name} app";
                  mainProgram = pkg.meta.mainProgram or name;
                };
                program = "${pkg}/bin/${pkg.meta.mainProgram or name}";
              };
            }
          ) (appNamesFor system)
        );
    in
    {

      packages = forEachSystem mkPackages;
      checks = forEachSystem mkChecks;
      apps = forEachSystem mkApps;

      overlays.default =
        final: prev:
        import ./. {
          pkgs = final;
          inherit (prev.stdenv.hostPlatform) isDarwin system;
        };

      legacyPackages = forEachSystem (
        system:
        let
          packages = packagesFor system;
        in
        builtins.listToAttrs (
          map (name: {
            name = name;
            value = packages.${name};
          }) (appNamesFor system)
        )
      );
    };
}
