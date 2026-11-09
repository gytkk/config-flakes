{ lib, stdenv }:

let
  version = "18.18.2";
  rev = "cb4801a7d6db666946a1b65fce5cdf50d2947b96";
  # Reuse upstream's locked Bun, Rust, and bun2nix toolchain with its package checks.
  upstream = builtins.getFlake "github:can1357/oh-my-pi/${rev}";
  package = upstream.packages.${stdenv.hostPlatform.system}.omp;
in
assert lib.assertMsg (
  package.version == version
) "OMP release version does not match its pinned source";
package
