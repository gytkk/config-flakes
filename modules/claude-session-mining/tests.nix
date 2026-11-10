let
  flake = builtins.getFlake (toString ../../..);
  lib = flake.inputs.nixpkgs.lib;
  mkConfig =
    {
      system,
      enable ? false,
      runnerPath ? null,
    }:
    flake.inputs.home-manager.lib.homeManagerConfiguration {
      pkgs = import flake.inputs.nixpkgs { inherit system; };
      modules = [
        ./default.nix
        {
          home = {
            username = "test";
            homeDirectory = "/Users/test";
            stateVersion = "25.11";
          };
          modules.claudeSessionMining = {
            inherit enable runnerPath;
          };
        }
      ];
    };
  darwin = mkConfig {
    system = "aarch64-darwin";
    enable = true;
    runnerPath = "/Users/test/workspace/gytkk-space/automation/mine-sessions.sh";
  };
  disabled = mkConfig { system = "x86_64-linux"; };
  linux = mkConfig {
    system = "x86_64-linux";
    enable = true;
    runnerPath = "/home/test/mine-sessions.sh";
  };
  missingPath = mkConfig {
    system = "aarch64-darwin";
    enable = true;
  };
  relativePath = mkConfig {
    system = "aarch64-darwin";
    enable = true;
    runnerPath = "workspace/mine-sessions.sh";
  };
  agent = darwin.config.launchd.agents.claude-session-mining;
  activation = darwin.config.home.activation.claudeMiningLogDir.data;
in
{
  valid =
    assert agent.enable;
    assert
      agent.config.ProgramArguments
      == [ "/Users/test/workspace/gytkk-space/automation/mine-sessions.sh" ];
    assert (builtins.head agent.config.StartCalendarInterval).Hour == 6;
    assert (builtins.head agent.config.StartCalendarInterval).Minute == 0;
    assert agent.config.ProcessType == "Background";
    assert agent.config.StandardOutPath == "/Users/test/Library/Logs/claude-mining/launchd.stdout";
    assert agent.config.StandardErrorPath == "/Users/test/Library/Logs/claude-mining/launchd.stderr";
    assert lib.hasInfix "mkdir -p /Users/test/Library/Logs/claude-mining" activation;
    assert lib.hasInfix "runner is missing or not executable" activation;
    "valid Darwin configuration";

  disabled =
    assert !(disabled.config.launchd.agents ? claude-session-mining);
    "disabled configuration";

  invalidLinux = linux.activationPackage;
  invalidMissingPath = missingPath.activationPackage;
  invalidRelativePath = relativePath.activationPackage;
}
