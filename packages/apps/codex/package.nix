{
  lib,
  stdenv,
  stdenvNoCC,
  fetchurl,
  autoPatchelfHook,
  jq,
  libcap,
  ncurses,
  openssl,
  zlib,
}:

let
  version = "0.170.0";

  platformMap = {
    "aarch64-darwin" = {
      target = "aarch64-apple-darwin";
      hash = "sha256-l1UkvxOu3RsSLY0OAMCe5nCRuPaIwLN0lD27PPIseJM=";
    };

    "x86_64-darwin" = {
      target = "x86_64-apple-darwin";
      hash = "sha256-LpcVJEYKVwuRPb3JtJ5enBRVbkTn9i8AuVtKjx4s3yU=";
    };

    "x86_64-linux" = {
      target = "x86_64-unknown-linux-musl";
      hash = "sha256-p3/4e+e6gJ1xVygnCnj1zjUn3AdSlBbR74kpuQuEdes=";
    };

    "aarch64-linux" = {
      target = "aarch64-unknown-linux-musl";
      hash = "sha256-P34a44xP4nMi4IK4VejuyTbQj6U41itxwH4qU/NOiEc=";
    };
  };

  platform =
    platformMap.${stdenvNoCC.hostPlatform.system}
      or (throw "Unsupported system: ${stdenvNoCC.hostPlatform.system}");

in
stdenvNoCC.mkDerivation {
  pname = "codex";
  inherit version;

  src = fetchurl {
    url = "https://github.com/openai/codex/releases/download/rust-v${version}/codex-package-${platform.target}.tar.gz";
    inherit (platform) hash;
  };

  dontUnpack = true;

  nativeBuildInputs = lib.optionals stdenvNoCC.hostPlatform.isLinux [ autoPatchelfHook ];

  buildInputs = lib.optionals stdenvNoCC.hostPlatform.isLinux [
    libcap
    ncurses
    openssl
    zlib
    stdenv.cc.cc.lib
  ];

  dontStrip = true;

  installPhase = ''
    runHook preInstall
    mkdir -p "$out"
    tar xzf "$src" -C "$out"
    runHook postInstall
  '';

  preFixup = lib.optionalString stdenvNoCC.hostPlatform.isLinux ''
    addAutoPatchelfSearchPath "$out/codex-resources/voice/lib"
  '';

  doInstallCheck = true;
  nativeInstallCheckInputs = [ jq ];
  installCheckPhase = ''
    runHook preInstallCheck
    bash ${./check-package.sh} "$out" "$src" "${version}" "${platform.target}"
    runHook postInstallCheck
  '';

  meta = {
    description = "Lightweight coding agent that runs in your terminal";
    homepage = "https://github.com/openai/codex";
    license = lib.licenses.asl20;
    platforms = builtins.attrNames platformMap;
    mainProgram = "codex";
  };
}
