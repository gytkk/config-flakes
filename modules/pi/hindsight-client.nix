{ pkgs }:
pkgs.applyPatches {
  name = "hindsight-pi-client-0.8.8";
  src = pkgs.fetchzip {
    name = "hindsight-coding-agents-0.8.8";
    url = "https://registry.npmjs.org/@vectorize-io/hindsight-coding-agents/-/hindsight-coding-agents-0.8.8.tgz";
    hash = "sha256-zuYTNIDn5XCuSnJjDKxpupjE440Hqw1YrPztLFDfA1s=";
  };
  # Home Manager exposes the companion skill through the Pi package manifest.
  postPatch = ''
    substituteInPlace dist/pi.js --replace-fail 'syncCompanionSkill(this.harness, { install: true });' ""
  '';
}
