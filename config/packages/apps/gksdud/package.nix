{
  lib,
  stdenvNoCC,
  fetchurl,
  unzip,
}:

stdenvNoCC.mkDerivation rec {
  pname = "gksdud";
  version = "1.6.0";

  src = fetchurl {
    url = "https://github.com/codingnoye/gksdud/releases/download/v${version}/gksdud-${version}-macos-universal.zip";
    hash = "sha256-eCNt5pWHzmrfrPoec8X5BZsauLEQTtJyLOayHoIEjiU=";
  };

  nativeBuildInputs = [ unzip ];
  sourceRoot = ".";
  dontBuild = true;
  # Preserve the upstream bundle's code signature.
  dontFixup = true;

  installPhase = ''
    runHook preInstall
    mkdir -p "$out/Applications" "$out/bin"
    cp -R gksdud.app "$out/Applications/"
    cat > "$out/bin/gksdud" <<EOF
    #!${stdenvNoCC.shell}
    exec /usr/bin/open -a "$out/Applications/gksdud.app" --args "\$@"
    EOF
    chmod +x "$out/bin/gksdud"
    runHook postInstall
  '';

  meta = {
    description = "macOS Korean and English input switching utility";
    homepage = "https://github.com/codingnoye/gksdud";
    license = lib.licenses.mit;
    platforms = lib.platforms.darwin;
    mainProgram = "gksdud";
  };
}
