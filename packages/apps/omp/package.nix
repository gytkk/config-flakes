{ lib, stdenv }:

let
  version = "18.18.6";
  rev = "9058ab9d229663e229e2867824ba6df6f21e9e59";
  # Reuse upstream's locked Bun, Rust, and bun2nix toolchain with its package checks.
  upstream = builtins.getFlake "github:can1357/oh-my-pi/${rev}";
  package = upstream.packages.${stdenv.hostPlatform.system}.omp;
in
assert lib.assertMsg (
  package.version == version
) "OMP release version does not match its pinned source";
package
