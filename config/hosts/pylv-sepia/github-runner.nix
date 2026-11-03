{
  config,
  lib,
  pkgs,
  ...
}:
{
  age.secrets.github-runner-ws-token.file = ../../secrets/github-runner-ws-token.age;

  services.github-runners.ws = {
    enable = true;
    url = "https://github.com/gytkk/ws";
    tokenFile = config.age.secrets.github-runner-ws-token.path;
    extraLabels = [ "pylv-sepia" ];
    # Re-registration after a token or workDir change otherwise fails on the existing runner name.
    replace = true;
    # The default work directory and HOME live in the /run tmpfs, which Rust builds fill.
    # Use the real DynamicUser path, not the /var/cache/github-runner/ws symlink: git resolves the
    # gitdir, so actions/checkout's includeIf credential entry for the symlink path never matches.
    workDir = "/var/cache/private/github-runner/ws";
    serviceOverrides = {
      CacheDirectory = [ "github-runner/ws" ];
      # The module binds a custom workDir onto itself, which makes the DynamicUser cache directory read-only.
      BindPaths = lib.mkForce [ ];
      # systemd mounts DynamicUser directories noexec; jobs run actions and rustup toolchains from here.
      ExecPaths = [ config.services.github-runners.ws.workDir ];
    };
    # Workflow tools that GitHub-hosted Ubuntu images provide.
    extraPackages = with pkgs; [
      curl
      diffutils
      findutils
      gawk
      gcc
      gnupatch
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
