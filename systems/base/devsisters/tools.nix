# Devsisters tools that need no agenix secret. Shared by the devsisters
# profile and jari workers.
{
  lib,
  pkgs,
  inputs,
  ...
}:

let
  inherit (pkgs.stdenv.hostPlatform) system;
in
{
  imports = [ ../../../modules/terraform ];

  home.packages = with pkgs; [
    # Authentication
    vault
    inputs.keycloak2aws.packages.${system}.kc2aws
    inputs.devsisters-wg.packages.${system}.wg-cli
    # wg-quick for the devsisters WireGuard profiles
    wireguard-tools

    # kubectl oidc-login plugin (int128/kubelogin)
    kubelogin-oidc

    # Ruby
    pkgs.stable-25_05.ruby_3_2

    # Databricks CLI (root nixpkgs의 Go 구현체)
    databricks-cli

    # Pantry CLI (teamdev.ai 배포 도구, main 빌드 바이너리, 수동 업데이트)
    pantry

    # Custom scripts
    (pkgs.writeShellScriptBin "sign" (builtins.readFile ./scripts/sign))
    (pkgs.writeShellScriptBin "dlg" (builtins.readFile ./scripts/login))
  ];

  # Terraform 모듈 설정
  modules.terraform = {
    enable = true;
    runEnv = {
      AWS_PROFILE = "saml";
    };
  };

  # Devsisters 특화 Git 설정
  programs.git.settings.user = {
    email = lib.mkForce "gyutak.kim@devsisters.com";
    name = lib.mkForce "Gyutak Kim";
  };

  home.sessionVariables = {
    VAULT_ADDR = "https://vault.devsisters.cloud";
    # SBT Java 호환성 설정
    SBT_OPTS = "-Xmx2G -XX:+UseG1GC";
  };

  # Add gem binaries to PATH, after the shared entries
  home.sessionPath = lib.mkAfter [
    "$HOME/.gem/ruby/3.2.0/bin"
  ];
}
