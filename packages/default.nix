{
  pkgs,
  isDarwin ? pkgs.stdenv.hostPlatform.isDarwin,
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
  claude-code = callPackage ./apps/claude-code/package.nix { };
  codex = callPackage ./apps/codex/package.nix { };
  codexbar = callPackage ./apps/codexbar/package.nix { };
  herdr = callPackage ./apps/herdr/package.nix { };
  herdr-core = callPackage ./apps/herdr-core/package.nix { };
  notion-cli = callPackage ./apps/notion-cli/package.nix { };
  ntn = notion-cli;
  pantry = callPackage ./apps/pantry/package.nix { };
  pi = callPackage ./apps/pi/package.nix { };
}
// (if isDarwin then { gksdud = callPackage ./apps/gksdud/package.nix { }; } else { })
