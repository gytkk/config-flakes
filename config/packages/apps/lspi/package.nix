{
  lib,
  stdenv,
  mkTarballCli,
  autoPatchelfHook,
}:

let
  version = "0.2.0";
in
(mkTarballCli {
  pname = "lspi";
  inherit version;
  platforms = {
    aarch64-darwin = {
      target = "aarch64-apple-darwin";
      hash = "sha256-L9qtvALT1E7zOVGhlUaoP589Mrx3TuGGahPMYCMoyOw=";
    };
    x86_64-darwin = {
      target = "x86_64-apple-darwin";
      hash = "sha256-rtj3ZF067QN3on1NMNrOrnRkHpfV+PsH9EBQ0ngJKFw=";
    };
    x86_64-linux = {
      target = "x86_64-unknown-linux-gnu";
      hash = "sha256-Ua2ZJir4PeheCmFotezz8ltPuhxgrukW5uLBSO6Odo4=";
    };
  };

  url =
    platform:
    "https://github.com/Latias94/lspi/releases/download/v${version}/lspi-${platform.target}.tar.xz";
  sourceRoot = platform: "lspi-${platform.target}";

  meta = {
    description = "Language server bridge for MCP clients";
    homepage = "https://github.com/Latias94/lspi";
    license = with lib.licenses; [
      mit
      asl20
    ];
    mainProgram = "lspi";
  };
}).overrideAttrs
  (_: {
    nativeBuildInputs = lib.optionals stdenv.hostPlatform.isLinux [ autoPatchelfHook ];
    buildInputs = lib.optionals stdenv.hostPlatform.isLinux [ stdenv.cc.cc.lib ];

    doInstallCheck = true;
    installCheckPhase = ''
      runHook preInstallCheck
      test "$("$out/bin/lspi" --version)" = "lspi ${version}"
      runHook postInstallCheck
    '';
  })
