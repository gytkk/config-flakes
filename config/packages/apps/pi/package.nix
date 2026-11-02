{ callPackage }:

let
  version = "1.4.0";
  # The upstream package reads lockfiles and the model catalog during evaluation.
  source = builtins.fetchTarball {
    url = "https://github.com/earendil-works/pi/archive/refs/tags/v${version}.tar.gz";
    sha256 = "sha256-N9NOpiiZTbOMDMV+CxvJAgtLjJJ1/8NJBGS0cFHkhsc=";
  };
in
callPackage "${source}/nix/package.nix" { inherit source; }
