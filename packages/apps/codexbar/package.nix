{
  lib,
  stdenvNoCC,
  fetchurl,
}:

let
  version = "0.84.0";

  platformMap = {
    "aarch64-darwin" = {
      target = "macos-arm64";
      hash = "sha256-d6MYxrkEmDcR8bfSSCEbG71Kd7XgOCWKeboiWGg/JgM=";
    };

    "x86_64-darwin" = {
      target = "macos-x86_64";
      hash = "sha256-s1RUOA1z6g126UBlM7/5lwY5Y+LFsxxqVXZ6pF1yUV0=";
    };

    "x86_64-linux" = {
      target = "linux-musl-x86_64";
      hash = "sha256-0OqATFZ+O1hVgj2i7TqMmN1CKrlYqWhKPXfRyc8l3NE=";
    };

    "aarch64-linux" = {
      target = "linux-musl-aarch64";
      hash = "sha256-+6y0Nq+Vzha+pF3Zs5Pp345H8KvvpdwjbfWysBUy0Hk=";
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
