{
  lib,
  stdenvNoCC,
  fetchurl,
}:

let
  version = "0.83.0";

  platformMap = {
    "aarch64-darwin" = {
      target = "macos-arm64";
      hash = "sha256-P7avlYKgsjLcFwjh2PE5hUQdvi50kDeRyQLR+ou3xKk=";
    };

    "x86_64-darwin" = {
      target = "macos-x86_64";
      hash = "sha256-KeJs+LB4G97WWXACjMy/YMDvUVOpoe4+XGWwD0ibYcc=";
    };

    "x86_64-linux" = {
      target = "linux-musl-x86_64";
      hash = "sha256-AORT7ObuLv2prsvYocR/Dupjka4cipIvAZueRm+AnlM=";
    };

    "aarch64-linux" = {
      target = "linux-musl-aarch64";
      hash = "sha256-lZzM5ySolFZY6ajdu6G5v0q8Qu6upnGkvQzAknBScXg=";
    };
  };

  platform =
    platformMap.${stdenvNoCC.hostPlatform.system}
      or (throw "Unsupported system: ${stdenvNoCC.hostPlatform.system}");

  src = fetchurl {
    url = "https://github.com/steipete/CodexBar/releases/download/v${version}/CodexBarCLI-v${version}-${platform.target}.tar.gz";
    hash = platform.hash;
  };
in
stdenvNoCC.mkDerivation {
  pname = "codexbar";
  inherit version src;

  dontStrip = true;
  sourceRoot = ".";

  installPhase = ''
    runHook preInstall
    mkdir -p $out/bin $out/lib/codexbar
    cp -R ./* $out/lib/codexbar/
    ln -s $out/lib/codexbar/CodexBarCLI $out/bin/codexbar
    runHook postInstall
  '';

  meta = {
    description = "CLI for Codex and Claude usage and cost data";
    homepage = "https://codex.bar";
    license = lib.licenses.mit;
    platforms = builtins.attrNames platformMap;
    mainProgram = "codexbar";
  };
}
