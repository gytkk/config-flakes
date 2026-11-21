{
  lib,
  stdenvNoCC,
  fetchurl,
}:

let
  version = "0.85.0";

  platformMap = {
    "aarch64-darwin" = {
      target = "macos-arm64";
      hash = "sha256-dBwtbxjIlbHCzPeYk/KH3qwdYuzssx9aM4Rz6x5p8wg=";
    };

    "x86_64-darwin" = {
      target = "macos-x86_64";
      hash = "sha256-2Xq5nFGV2WOjOF48qMbj87uoRr9LgPpw3Wf1i8n8ar4=";
    };

    "x86_64-linux" = {
      target = "linux-musl-x86_64";
      hash = "sha256-ZRk/3HjyPUB1kjGLE7MHeCrqh3IapOsl5SWqp67+Gdw=";
    };

    "aarch64-linux" = {
      target = "linux-musl-aarch64";
      hash = "sha256-lRbFMo19KM8a3XG1b5JuR9TCGejRYTSWXbaGWlM90c8=";
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
