{
  lib,
  fetchurl,
  stdenvNoCC,
}:
{
  pname,
  version,
  platforms,
  url,
  sourceRoot ? (_: "."),
  extraInstall ? "",
  meta,
}:
let
  platform =
    platforms.${stdenvNoCC.hostPlatform.system}
      or (throw "Unsupported system for ${pname}: ${stdenvNoCC.hostPlatform.system}");
in
stdenvNoCC.mkDerivation {
  inherit pname version;

  src = fetchurl {
    url = url platform;
    inherit (platform) hash;
  };

  sourceRoot = sourceRoot platform;
  dontConfigure = true;
  dontBuild = true;

  installPhase = ''
    runHook preInstall

    install -Dm755 ${lib.escapeShellArg meta.mainProgram} "$out/bin/${meta.mainProgram}"
  ''
  + extraInstall
  + ''

    runHook postInstall
  '';

  meta = meta // {
    platforms = builtins.attrNames platforms;
  };
}
