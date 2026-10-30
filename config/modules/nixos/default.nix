{ inputs, ... }:

{
  imports = [
    inputs.agenix.nixosModules.default
    ./baseline.nix
    ./nix-gc.nix
    ./remote-access.nix
    ./secrets.nix
    ./user.nix
    ../codex/system.nix
  ];
}
