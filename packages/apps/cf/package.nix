{
  lib,
  stdenv,
  buildNpmPackage,
  fetchurl,
  jq,
  autoPatchelfHook,
  versionCheckHook,
}:

let
  # Fetch during evaluation so --no-build does not need a source derivation.
  nodePkgs = import (builtins.fetchTarball {
    url = "https://github.com/NixOS/nixpkgs/archive/752cbac9446602fbb78bdd1c3597455a3dd38432.tar.gz";
    sha256 = "sha256-P4nss8/EUu51hOQDdkso+pCQBdt0DT/lrfXWaZEDnEA=";
  }) { system = stdenv.hostPlatform.system; };
in
buildNpmPackage rec {
  pname = "cf";
  version = "1.0.0-beta.21";

  src = fetchurl {
    url = "https://registry.npmjs.org/cf/-/cf-${version}.tgz";
    hash = "sha512-rJqfa8zW7pN0m/fmc11xT3GzcXrqwhtZ9pizO3LZyZJjoOPR+h/us/+ajK0tXPhL+jHqaW/39ODQ481GDUyziQ==";
  };

  sourceRoot = "package";
  npmDepsHash = "sha256-Yz7HwSwXo2h/JQhTSU5SWkJMRmtbsdle+gyJ3/cEWGY=";
  nodejs = nodePkgs.nodejs_26;
  dontNpmBuild = true;
  npmFlags = [ "--ignore-scripts" ];
  npmInstallFlags = [ "--omit=dev" ];

  postPatch = ''
    # The published bundle lists development-only file: dependencies absent from npm.
    ${lib.getExe jq} 'del(.devDependencies)' package.json > package.json.tmp
    mv package.json.tmp package.json
    cp ${./package-lock.json} package-lock.json
  '';

  nativeBuildInputs = lib.optionals stdenv.hostPlatform.isLinux [ autoPatchelfHook ];
  buildInputs = lib.optionals stdenv.hostPlatform.isLinux [ stdenv.cc.cc.lib ];

  doInstallCheck = true;
  nativeInstallCheckInputs = [ versionCheckHook ];
  versionCheckProgram = "${placeholder "out"}/bin/cf";
  versionCheckProgramArg = "--version";

  meta = {
    description = "Official Cloudflare CLI for resource management and Workers";
    homepage = "https://developers.cloudflare.com/cf/";
    license = with lib.licenses; [
      mit
      asl20
    ];
    platforms = [
      "aarch64-darwin"
      "aarch64-linux"
      "x86_64-linux"
    ];
    mainProgram = "cf";
  };
}
