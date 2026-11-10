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

  runtimeGoldenHashes = import ./agent-core/nix/runtime-golden-hashes.nix;
  mkAgentCoreOutputs =
    systemPkgs: golden:
    let
      render = import ./agent-core/nix/render.nix { pkgs = systemPkgs; };
      hashFor = runtime: if golden then runtimeGoldenHashes.${runtime} else null;
    in
    {
      claude = render {
        outputHash = hashFor "claude";
        runtime = "claude";
      };
      codex = render {
        outputHash = hashFor "codex";
        runtime = "codex";
      };
      pi = render {
        outputHash = hashFor "pi";
        runtime = "pi";
      };
    };
  agentCoreOutputs = builtins.mapAttrs (_: systemPkgs: mkAgentCoreOutputs systemPkgs false) pkgs;
  agentCoreGoldenOutputs = builtins.mapAttrs (_: systemPkgs: mkAgentCoreOutputs systemPkgs true) pkgs;
  appPackages = builtins.mapAttrs (_: systemPkgs: import ./packages { pkgs = systemPkgs; }) pkgs;

  agentCoreSourcesMatch =
    system:
    let
      output = agentCoreOutputs.${system};
      homeName = if system == "aarch64-darwin" then "devsisters-macbook" else "pylv-denim";
      homeFiles = homeConfigurations.${homeName}.config.home.file;
      skillActivation = homeConfigurations.${homeName}.config.home.activation.installAgentSkills;
      commonMatches = [
        (toString homeFiles.".claude/CLAUDE.md".source == "${output.claude}/CLAUDE.md")
        (!(builtins.hasAttr ".claude/skills" homeFiles))
        (toString homeFiles.".codex/AGENTS.md".source == "${output.codex}/AGENTS.md")
        (toString homeFiles.".pi/agent/AGENTS.md".source == "${output.pi}/AGENTS.md")
        (toString homeFiles.".pi/agent/APPEND_SYSTEM.md".source == "${output.pi}/APPEND_SYSTEM.md")
        (!(builtins.hasAttr ".pi/agent/skills" homeFiles))
        (!(builtins.hasAttr ".agents/skills" homeFiles))
        (nixpkgs.lib.hasInfix "install --apply --target shared --target claude" skillActivation.data)
        (!(nixpkgs.lib.hasInfix "--target openclaw" skillActivation.data))
        (builtins.elem "linkGeneration" skillActivation.after)
        (builtins.elem "codexUserConfig" skillActivation.after)
        (
          !(nixpkgs.lib.hasInfix "/etc/codex/skills"
            homeConfigurations.${homeName}.config.home.activation.codexUserConfig.data
          )
        )
      ];
      onyx = nixosConfigurations.pylv-onyx.config;
      onyxHomeFiles = onyx.home-manager.users.gytkk.home.file;
      onyxSkillActivation = onyx.home-manager.users.gytkk.home.activation.installAgentSkills;
      onyxOpenClawSystemdDropInMaterialization =
        onyx.home-manager.users.gytkk.home.activation.materializeOpenClawSystemdDropIn.data;
      linuxMatches = [
        (nixpkgs.lib.hasInfix "install --apply --target shared --target claude" onyxSkillActivation.data)
        (!(nixpkgs.lib.hasInfix "--target openclaw" onyxSkillActivation.data))
        (builtins.elem "linkGeneration" onyxSkillActivation.after)
        (builtins.elem "codexUserConfig" onyxSkillActivation.after)
        (!(builtins.hasAttr ".agents/skills" onyxHomeFiles))
        (!(builtins.hasAttr ".claude/skills" onyxHomeFiles))
        (!(builtins.hasAttr ".pi/agent/skills" onyxHomeFiles))
        (
          !(builtins.hasAttr "materializeOpenClawRuntimeTrees" onyx.home-manager.users.gytkk.home.activation)
        )
        (builtins.all (path: !(builtins.hasAttr path onyxHomeFiles)) [
          ".local/share/openclaw/agent-core/AGENTS.core.md"
          ".local/share/openclaw/agent-core/skills"
          ".local/share/openclaw/extensions/agent-core-context"
          ".local/share/openclaw/extensions/agent-session-record"
          ".openclaw/skills"
          ".openclaw/managed/agent-core/AGENTS.core.md"
          ".openclaw/extensions/agent-core-context"
          ".openclaw/extensions/agent-session-record"
        ])
        (
          !(builtins.hasAttr "systemd/user/openclaw-gateway.service.d/20-nix-runtime.conf" onyx.home-manager.users.gytkk.xdg.configFile)
        )
        (nixpkgs.lib.hasInfix "/home/gytkk/.config/systemd/user/openclaw-gateway.service.d/20-nix-runtime.conf" onyxOpenClawSystemdDropInMaterialization)
        (nixpkgs.lib.hasInfix "install -D -m 0644" onyxOpenClawSystemdDropInMaterialization)
        (!(builtins.hasAttr "codex/skills" onyx.environment.etc))
      ];
    in
    builtins.all (matches: matches) (
      commonMatches ++ nixpkgs.lib.optionals (system == "x86_64-linux") linuxMatches
    );

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

  checks = builtins.mapAttrs (system: systemPkgs: {
    agent-core = systemPkgs.agent-core;
    agent-core-renders =
      assert agentCoreSourcesMatch system;
      systemPkgs.runCommand "agent-core-render-check"
        {
          nativeBuildInputs = with systemPkgs; [
            agent-core
            jq
            nodejs
            ripgrep
          ];
        }
        ''
          agent-core check
          bash ${./modules/agent-skills}/tests/activation.test.sh ${
            systemPkgs.writeText "agent-skills-activation.sh" (
              builtins.unsafeDiscardStringContext
                homeConfigurations.${
                  if system == "aarch64-darwin" then "devsisters-macbook" else "pylv-denim"
                }.config.home.activation.installAgentSkills.data
            )
          } shared claude
          ${nixpkgs.lib.optionalString (system == "x86_64-linux") ''
            bash ${./modules/agent-skills}/tests/activation.test.sh ${systemPkgs.writeText "onyx-agent-skills-activation.sh" (builtins.unsafeDiscardStringContext nixosConfigurations.pylv-onyx.config.home-manager.users.gytkk.home.activation.installAgentSkills.data)} shared claude
          ''}
          test -d ${agentCoreGoldenOutputs.${system}.claude}/skills
          test -d ${agentCoreGoldenOutputs.${system}.codex}/skills/devils-advocate
          test -d ${agentCoreGoldenOutputs.${system}.pi}/skills/pi-agent
          touch "$out"
        '';
  }) pkgs;
in
{
  packages = defaultPackages;
  apps = defaultApps;
  inherit checks;

  inherit homeConfigurations nixosConfigurations;
}
