{ pkgs, lib, ... }:
let
  inventory = import ../inventory.nix;
in
{
  # Home Manager 환경 설정 (모든 호스트)
  allEnvironments = inventory;

  # NixOS 호스트 설정
  allHosts = lib.filterAttrs (_: config: config.kind == "nixos") inventory;
}
