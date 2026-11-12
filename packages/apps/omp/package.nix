{ lib, stdenv }:

let
  version = "18.18.9";
  rev = "50da2d357039404a81d7c64622740ce13e4c54b4";
  # Reuse upstream's locked Bun, Rust, and bun2nix toolchain with its package checks.
  upstream = builtins.getFlake "github:can1357/oh-my-pi/${rev}";
  package = upstream.packages.${stdenv.hostPlatform.system}.omp;
in
assert lib.assertMsg (
  package.version == version
) "OMP release version does not match its pinned source";
package
