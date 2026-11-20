flake:
let
  sources = builtins.genericClosure {
    startSet = [
      {
        key = flake.outPath;
        value = flake;
      }
    ];
    operator =
      node:
      map (value: {
        key = value.outPath;
        inherit value;
      }) (builtins.attrValues (node.value.inputs or { }));
  };
in
builtins.toFile "devsisters-jari.nix" ''
  # Retain the locked input sources when Nix collects garbage.
  ${builtins.concatStringsSep "\n" (map (source: "# ${source.key}") sources)}
  (builtins.getFlake "path:${flake.outPath}").nixosModules.devsisters-jari
''
