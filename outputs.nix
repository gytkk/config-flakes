{ nixpkgs, ... }@inputs:
let
  # 라이브러리 import
  lib = import ./lib { inherit inputs nixpkgs; };

  # 시스템별 패키지
  pkgs = lib.builders.mkSystemPkgs [
    "x86_64-linux"
    "aarch64-darwin"
  ];

  # 환경별 설정 (라이브러리에서 자동 로드)
  environmentConfigs = lib.environments.allEnvironments;
  hostConfigs = lib.environments.allHosts;

  # 홈 설정 생성 함수
  mkHomeConfig = lib.builders.mkHomeConfig;

  # NixOS 설정 생성 함수
  mkNixOSConfig = lib.builders.mkNixOSConfig;

  homeConfigurations = builtins.mapAttrs mkHomeConfig environmentConfigs;
  nixosConfigurations = builtins.mapAttrs mkNixOSConfig hostConfigs;

  appPackages = builtins.mapAttrs (_: systemPkgs: import ./packages { pkgs = systemPkgs; }) pkgs;

  mkDefaultCompatPackage =
    system: systemPkgs:
    let
      recommendedTargets =
        if system == "aarch64-darwin" then
          ''
            nix build .#homeConfigurations.devsisters-macbook.activationPackage
            nix build .#homeConfigurations.devsisters-macstudio.activationPackage
          ''
        else
          ''
            nix build .#nixosConfigurations.pylv-onyx.config.system.build.toplevel
            nix build .#nixosConfigurations.pylv-sepia.config.system.build.toplevel
          '';
    in
    systemPkgs.writeShellScriptBin "config-flakes" ''
      cat <<'EOF'
      This flake does not expose a single real build target by default.
      Use an explicit attribute instead.

      Recommended targets:
      ${recommendedTargets}

      See also:
        nix flake show
      EOF
    '';

  defaultPackages = builtins.mapAttrs (
    system: systemPkgs:
    {
      default = mkDefaultCompatPackage system systemPkgs;
      agent-core = systemPkgs.agent-core;
    }
    // appPackages.${system}
  ) pkgs;

  defaultApps = builtins.mapAttrs (system: _: {
    default = {
      type = "app";
      program = "${defaultPackages.${system}.default}/bin/config-flakes";
    };
    agent-core = {
      type = "app";
      program = "${defaultPackages.${system}.agent-core}/bin/agent-core";
    };
  }) pkgs;

  checks = import ./agent-core/nix/checks.nix {
    inherit pkgs homeConfigurations nixosConfigurations;
    lib = nixpkgs.lib;
    activationTests = ./modules/agent-skills/tests;
  };
in
{
  packages = defaultPackages;
  apps = defaultApps;
  inherit checks;

  inherit homeConfigurations nixosConfigurations;
}
