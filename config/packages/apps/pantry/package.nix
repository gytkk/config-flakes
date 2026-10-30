{
  lib,
  fetchurl,
  stdenvNoCC,
}:

let
  version = "main-69de978b5555";

  platforms = {
    x86_64-linux = {
      asset = "linux-amd64";
      hash = "sha256-oiM1W9vaAjk80OHF9Oz3s/O0PI92zNI+5yL9fDpD6xg=";
    };
    aarch64-linux = {
      asset = "linux-arm64";
      hash = "sha256-mIY+m7CmIPkSyi1V0zsSMMquDituwXQgmJ5C4ZaKcCw=";
    };
    x86_64-darwin = {
      asset = "darwin-amd64";
      hash = "sha256-AqOCP1exlTWoCc5lcRKYNPz7TmYATepENN67HvfyUy4=";
    };
    aarch64-darwin = {
      asset = "darwin-arm64";
      hash = "sha256-jZvdsmnyVi0yhyBpQz0Mqq8D8QzrY2DI9dJKb1gMGJc=";
    };
  };

  platform =
    platforms.${stdenvNoCC.hostPlatform.system}
      or (throw "Unsupported system for pantry: ${stdenvNoCC.hostPlatform.system}");
in
stdenvNoCC.mkDerivation {
  pname = "pantry";
  inherit version;

  # Each build is published as a bare executable, not an archive.
  src = fetchurl {
    url = "https://devsisters-vibe-static-public.s3.ap-northeast-2.amazonaws.com/pantry/cli/${version}/${platform.asset}/pantry";
    inherit (platform) hash;
  };

  dontUnpack = true;
  dontConfigure = true;
  dontBuild = true;

  installPhase = ''
    runHook preInstall

    install -Dm755 "$src" "$out/bin/pantry"

    runHook postInstall
  '';

  meta = {
    description = "CLI for the Devsisters teamdev.ai deployment platform";
    homepage = "https://github.com/devsisters/pantry";
    license = lib.licenses.unfree;
    mainProgram = "pantry";
    platforms = builtins.attrNames platforms;
  };
}
