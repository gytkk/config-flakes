{ pkgs, lib, ... }:
let
  inventory = import ../systems/inventory.nix;
in
{
  # Image-owned hosts consume a NixOS module instead of a standalone generation.
  allEnvironments = lib.filterAttrs (_: config: config.kind != "nixos-module") inventory;

  # NixOS 호스트 설정
  allHosts = lib.filterAttrs (_: config: config.kind == "nixos") inventory;
}
