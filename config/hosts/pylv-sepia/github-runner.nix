{ config, pkgs, ... }:
{
  age.secrets.github-runner-ws-token.file = ../../secrets/github-runner-ws-token.age;

  services.github-runners.ws = {
    enable = true;
    url = "https://github.com/gytkk/ws";
    tokenFile = config.age.secrets.github-runner-ws-token.path;
    extraLabels = [ "pylv-sepia" ];
    # Workflow tools that GitHub-hosted Ubuntu images provide.
    extraPackages = with pkgs; [
      curl
      diffutils
      findutils
      gawk
      gcc
      gnused
      jq
      perl
      ripgrep
      rustup
    ];
    # Lets binaries that actions download, such as setup-node's Node.js, run through nix-ld.
    extraEnvironment = {
      inherit (config.environment.sessionVariables) NIX_LD NIX_LD_LIBRARY_PATH;
    };
  };
}
