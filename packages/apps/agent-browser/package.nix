{
  lib,
  stdenv,
  stdenvNoCC,
  fetchurl,
  patchelf,
  autoPatchelfHook,
  makeWrapper,
  playwright-driver,
}:

let
  version = "0.39.3";

  platformMap = {
    "aarch64-darwin" = {
      suffix = "darwin-arm64";
      hash = "sha256-h7qxyUrMk9Zv3O7y0PiU/82zEGVALWd7Zial8i2yUXw=";
    };

    "x86_64-darwin" = {
      suffix = "darwin-x64";
      hash = "sha256-uN2fcLxolZpQJAfBP9c5TXZHoS1nQMRnENlOlKkJBKQ=";
    };

    "x86_64-linux" = {
      suffix = "linux-x64";
      hash = "sha256-JigzjflG4YansCZ9FufltevlZ628GAhMWVBCJ/BbJ08=";
    };

    "aarch64-linux" = {
      suffix = "linux-arm64";
      hash = "sha256-b4aw2Lotkr1S9y7YNCS1Iy2PVy5/dbeOYzGmTLa3Q54=";
    };
  };

  platform =
    platformMap.${stdenvNoCC.hostPlatform.system}
      or (throw "Unsupported system: ${stdenvNoCC.hostPlatform.system}");

  src = fetchurl {
    url = "https://github.com/vercel-labs/agent-browser/releases/download/v${version}/agent-browser-${platform.suffix}";
    hash = platform.hash;
  };
in
stdenvNoCC.mkDerivation {
  pname = "agent-browser";
  inherit version;

  dontUnpack = true;

  nativeBuildInputs = [ makeWrapper ] ++ lib.optionals stdenvNoCC.hostPlatform.isLinux [ patchelf ];

  dontAutoPatchelf = true;
  dontPatchELF = true;
  dontStrip = true;

  installPhase = ''
    runHook preInstall
    install -Dm755 ${src} $out/bin/agent-browser
  ''
  + lib.optionalString stdenvNoCC.hostPlatform.isLinux ''
    patchelf --set-interpreter "$(cat ${stdenv.cc}/nix-support/dynamic-linker)" $out/bin/agent-browser
  ''
  + ''
    wrapProgram $out/bin/agent-browser \
      --set PLAYWRIGHT_BROWSERS_PATH "${playwright-driver.browsers}"
    runHook postInstall
  '';

  meta = {
    description = "Browser automation CLI for AI agents";
    homepage = "https://github.com/vercel-labs/agent-browser";
    license = lib.licenses.asl20;
    platforms = builtins.attrNames platformMap;
    mainProgram = "agent-browser";
  };
}
