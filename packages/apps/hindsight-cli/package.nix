{
  lib,
  stdenvNoCC,
  fetchurl,
  autoPatchelfHook,
  openssl,
  stdenv,
}:
let
  platforms = {
    aarch64-darwin = {
      asset = "darwin-arm64";
      hash = "008defb04913cea36eeefa85cfbf4c163aee386d4b00e13c8a68ee2f84506ec3";
    };
    x86_64-linux = {
      asset = "linux-amd64";
      hash = "e3b7c83fd4eab3ac0614199ce888a636c783348aa5a001acc83e364b7368f96e";
    };
    aarch64-linux = {
      asset = "linux-arm64";
      hash = "2ee2111000964a59d0d83b063728653b711f974772f5b79c9342c8b4d4ba86dd";
    };
  };
  platform = platforms.${stdenv.hostPlatform.system};
in
stdenvNoCC.mkDerivation (finalAttrs: {
  pname = "hindsight-cli";
  version = "1.2.3";
  src = fetchurl {
    url = "https://github.com/vectorize-io/hindsight/releases/download/v${finalAttrs.version}/hindsight-${platform.asset}";
    sha256 = platform.hash;
  };
  nativeBuildInputs = lib.optionals stdenv.hostPlatform.isLinux [ autoPatchelfHook ];
  buildInputs = lib.optionals stdenv.hostPlatform.isLinux [
    openssl
    stdenv.cc.cc.lib
  ];
  dontUnpack = true;
  installPhase = ''
    install -Dm755 "$src" "$out/bin/hindsight"
  '';
  meta = {
    mainProgram = "hindsight";
    platforms = builtins.attrNames platforms;
    description = "Command-line client for Hindsight memory banks";
    license = lib.licenses.mit;
  };
})
