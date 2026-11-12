let
  flake = builtins.getFlake (toString ../..);
  inherit (flake) inputs;
  lib = inputs.nixpkgs.lib;
  agenixModule = builtins.toFile "agenix-home-test.nix" (
    import ../../lib/agenix-home.nix {
      inherit lib;
      source = builtins.readFile "${inputs.agenix}/modules/age-home.nix";
    }
  );
  mkConfig =
    system: secrets:
    inputs.home-manager.lib.homeManagerConfiguration {
      pkgs = import inputs.nixpkgs { inherit system; };
      modules = [
        agenixModule
        ./default.nix
        {
          home = {
            username = "test";
            homeDirectory = "/Users/test";
            stateVersion = "26.05";
          };
          age = {
            inherit secrets;
            identityPaths = [ "/Users/test/.ssh/test-identity" ];
          };
        }
      ];
    };
  # This file is only an evaluation fixture, never a decryption input at runtime.
  secrets.test.file = ./tests.nix;
  darwin = (mkConfig "aarch64-darwin" secrets).config;
  empty = (mkConfig "aarch64-darwin" { }).config;
  linux = (mkConfig "x86_64-linux" secrets).config;
  agent = darwin.launchd.agents.activate-agenix.config;
  activation = darwin.home.activation.writeAgenixLaunchdWrapper;
in
assert agent.ProgramArguments == [ "/Users/test/.local/state/agenix-launchd-wrapper" ];
assert agent.KeepAlive == null;
assert agent.RunAtLoad;
assert darwin.age.identityPaths == [ "/Users/test/.ssh/test-identity" ];
assert darwin.age.secretsDir == "/Users/test/.local/state/agenix";
assert darwin.age.secretsMountPoint == "/Users/test/.local/state/agenix.d";
assert darwin.age.secrets.test.mode == "0400";
assert activation.before == [ "setupLaunchAgents" ];
assert activation.after == [ "writeBoundary" ];
assert lib.hasInfix (builtins.unsafeDiscardStringContext darwin.age.mountingScript) activation.data;
assert builtins.hasContext activation.data;
assert
  builtins.intersectAttrs (builtins.getContext darwin.age.mountingScript) (
    builtins.getContext activation.data
  ) == builtins.getContext darwin.age.mountingScript;
assert !(empty.launchd.agents ? activate-agenix);
assert !(empty.home.activation ? writeAgenixLaunchdWrapper);
assert !(linux.home.activation ? writeAgenixLaunchdWrapper);
assert linux.systemd.user.services.agenix.Service.ExecStart == [ linux.age.mountingScript ];
"agenix Darwin, empty secrets, and Linux contracts passed"
