{
  pkgs,
  isDarwin ? pkgs.stdenv.hostPlatform.isDarwin,
  system ? pkgs.stdenv.hostPlatform.system,
}:
let
  callPackage = pkgs.lib.callPackageWith (
    pkgs
    // {
      mkTarballCli = pkgs.callPackage ./lib/mk-tarball-cli.nix { };
    }
  );
in
rec {
  agent-browser = callPackage ./apps/agent-browser/package.nix { };
  cf = callPackage ./apps/cf/package.nix { };
  claude-code = callPackage ./apps/claude-code/package.nix { };
  codex = callPackage ./apps/codex/package.nix { };
  codexbar = callPackage ./apps/codexbar/package.nix { };
  databricks-cli = callPackage ./apps/databricks-cli/package.nix { };
  herdr = callPackage ./apps/herdr/package.nix { };
  herdr-annotate = callPackage ./apps/herdr-annotate/package.nix { };
  herdr-auto-title = callPackage ./apps/herdr-auto-title/package.nix { };
  notion-cli = callPackage ./apps/notion-cli/package.nix { };
  ntn = notion-cli;
  pantry = callPackage ./apps/pantry/package.nix { };
  pi = callPackage ./apps/pi/package.nix { };
  pup = callPackage ./apps/pup/package.nix { };
}
// (if isDarwin then { gksdud = callPackage ./apps/gksdud/package.nix { }; } else { })
// (
  if
    builtins.elem system [
      "aarch64-darwin"
      "x86_64-darwin"
      "x86_64-linux"
    ]
  then
    {
      lspi = callPackage ./apps/lspi/package.nix { };
    }
  else
    { }
)
