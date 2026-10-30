{
  lib,
  stdenvNoCC,
  fetchurl,
}:

let
  version = "0.16.0";
  platforms = {
    aarch64-darwin = {
      target = "aarch64-macos";
      hash = "sha256-sj1w3qqHm1wtSG7TMW9+qlPoSs9vycx0feFSRQ1AFIk=";
    };
    x86_64-darwin = {
      target = "x86_64-macos";
      hash = "sha256-A4dVftGHe8ai4YAsg5GVO63bp2CBh2MBxSL1KXe1K6c=";
    };
    x86_64-linux = {
      target = "x86_64-linux";
      hash = "sha256-cOSWZKdDdLSLUebz/fv0N/Y5XUJQkFBYi9SavlK6PQA=";
    };
    aarch64-linux = {
      target = "aarch64-linux";
      hash = "sha256-6ksJv7IuxvbGzqxXq2PvtrRuF6sI0h9p86SLOOFTTxc=";
    };
  };
  platform = platforms.${stdenvNoCC.hostPlatform.system};
in
stdenvNoCC.mkDerivation {
  pname = "herdr-zig";
  inherit version;
  src = fetchurl {
    url = "https://ziglang.org/download/${version}/zig-${platform.target}-${version}.tar.xz";
    inherit (platform) hash;
  };

  dontBuild = true;
  dontStrip = true;

  installPhase = ''
    runHook preInstall
    mkdir -p $out/bin
    cp -r zig lib LICENSE $out/
    ln -s $out/zig $out/bin/zig
    runHook postInstall
  '';

  meta = {
    description = "Pinned Zig build tool for Herdr on older nixpkgs";
    homepage = "https://ziglang.org";
    license = lib.licenses.mit;
    platforms = builtins.attrNames platforms;
    mainProgram = "zig";
  };
}
