{
  lib,
  stdenvNoCC,
  fetchurl,
}:

let
  version = "0.82.0";

  platformMap = {
    "aarch64-darwin" = {
      target = "macos-arm64";
      hash = "sha256-Xm2SL7671Bp0i8kGUAdAUpJl5lr7x//SzoyHlPxe33Y=";
    };

    "x86_64-darwin" = {
      target = "macos-x86_64";
      hash = "sha256-rKiLm5Fhl+Xg26M8DiqOePJ9SkB2Cs+ElQkGC2MM6gw=";
    };

    "x86_64-linux" = {
      target = "linux-musl-x86_64";
      hash = "sha256-Rxfc0KueRcVzcqBMAaPiJOWIPqPPqnHIpHWy+Q/30Do=";
    };

    "aarch64-linux" = {
      target = "linux-musl-aarch64";
      hash = "sha256-mdKmE4E+GLhW+gPS0wC4MKCDZJeQdrICug+S0w8WAGg=";
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
