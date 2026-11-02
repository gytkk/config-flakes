{
  callPackage,
  lib,
  stdenv,
}:

let
  version = "1.4.0";
  # The upstream package reads lockfiles and the model catalog during evaluation.
  source = builtins.fetchTarball {
    url = "https://github.com/earendil-works/pi/archive/refs/tags/v${version}.tar.gz";
    sha256 = "sha256-N9NOpiiZTbOMDMV+CxvJAgtLjJJ1/8NJBGS0cFHkhsc=";
  };
in
callPackage "${source}/nix/package.nix" {
  inherit source;
  # Patch the workspace build, not source metadata read during evaluation or the final install lock.
  stdenv = stdenv // {
    mkDerivation =
      attrs:
      stdenv.mkDerivation (
        attrs
        // lib.optionalAttrs (attrs.pname == "pi-workspace-packages") {
          patches = (attrs.patches or [ ]) ++ import ./patches.nix;
          postPatch = (attrs.postPatch or "") + ''
            cp ${./herdr-file-opener.ts} packages/coding-agent/src/utils/herdr-file-opener.ts
          '';
        }
      );
  };
}
