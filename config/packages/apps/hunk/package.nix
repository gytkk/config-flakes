{ lib, stdenv }:

let
  version = "0.24.1";
  rev = "9cca76fbf700217b574dfcdd4fe56c5b4388240b";
  # Keep upstream's locked Bun/bun2nix toolchain and bundled TUI entrypoints together.
  upstream = builtins.getFlake "github:modem-dev/hunk/${rev}";
  package = upstream.packages.${stdenv.hostPlatform.system}.hunk;
in
assert lib.assertMsg (
  package.version == version
) "Hunk release version does not match its pinned source";
package
