{ inputs }:
let
  agenixModule = builtins.toFile "agenix-home.nix" (
    import ./patch.nix {
      lib = inputs.nixpkgs.lib;
      source = builtins.readFile "${inputs.agenix}/modules/age-home.nix";
    }
  );
in
{
  imports = [
    agenixModule
    ./darwin.nix
  ];
}
