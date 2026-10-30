{ callPackage, lib }:

let
  version = "1.4.0";
  # The upstream package reads lockfiles and the model catalog during evaluation.
  source = builtins.fetchTarball {
    url = "https://github.com/earendil-works/pi/archive/refs/tags/v${version}.tar.gz";
    sha256 = "sha256-N9NOpiiZTbOMDMV+CxvJAgtLjJJ1/8NJBGS0cFHkhsc=";
  };
in
(callPackage "${source}/nix/package.nix" { inherit source; }).overrideAttrs (old: {
  # Upstream's smoke test still targets esbuild, but extensions use jiti.
  installCheckPhase =
    lib.replaceStrings
      [
        "require('$out/lib/pi/node_modules/esbuild').transformSync('const value: number = 1', { loader: 'ts' })"
      ]
      [
        ''
          const assert = require('node:assert/strict');
          const { createJiti } = require('$out/lib/pi/node_modules/jiti');
          const jiti = createJiti('$out/lib/pi/package.json', { fsCache: false, moduleCache: false, tryNative: false });
          assert.equal(jiti.evalModule('const value: number = 1; export default value', { filename: '$TMPDIR/pi-install-check.ts' }).default, 1);
        ''
      ]
      old.installCheckPhase;
})
