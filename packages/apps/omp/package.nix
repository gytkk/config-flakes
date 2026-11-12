{ lib, stdenv }:

let
  version = "18.19.0";
  rev = "04cd1a7549a02d62ad1d24826198d52507595683";
  # Reuse upstream's locked Bun, Rust, and bun2nix toolchain with its package checks.
  upstream = builtins.getFlake "github:can1357/oh-my-pi/${rev}";
  package = upstream.packages.${stdenv.hostPlatform.system}.omp;
in
assert lib.assertMsg (
  package.version == version
) "OMP release version does not match its pinned source";
package
