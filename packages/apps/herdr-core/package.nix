{
  lib,
  stdenvNoCC,
  fetchurl,
}:
let
  version = "0.1.20";
  platforms = {
    aarch64-darwin = {
      target = "macos-aarch64";
      hash = "sha256-5YmF0/726QnzPTxaRCbkiYyBVkpVfUY2WRzsnjoGryY=";
    };
    x86_64-darwin = {
      target = "macos-x86_64";
      hash = "sha256-NsAzHD/+oP+q2hlbiaPHSW9/P8W/NeRgXBnITlwgWlY=";
    };
    aarch64-linux = {
      target = "linux-aarch64";
      hash = "sha256-XzA3N50VzHbPxy0v0aQmLqHFsk3yeL98/Yrc/3uRSGI=";
    };
    x86_64-linux = {
      target = "linux-x86_64";
      hash = "sha256-rGuGiQxFGjdYRiCqpDi3G8DQvUMKLnwdfglfBSxBTnQ=";
    };
  };
  platform = platforms.${stdenvNoCC.hostPlatform.system};
in
stdenvNoCC.mkDerivation {
  pname = "herdr-core";
  inherit version;

  src = fetchurl {
    url = "https://releases.herdr.dev/core/${version}/herdr-core-${platform.target}.tar.gz";
    inherit (platform) hash;
  };

  sourceRoot = ".";
  dontConfigure = true;
  dontBuild = true;
  # Linux binaries are static; macOS bundles must retain their upstream signature.
  dontFixup = true;

  installPhase = ''
    runHook preInstall
    mkdir -p "$out/bin" "$out/share/doc/herdr-core"
  ''
  + lib.optionalString stdenvNoCC.hostPlatform.isDarwin ''
    mkdir -p "$out/Applications"
    cp -R "Herdr Core.app" "$out/Applications/"
    ln -s "$out/Applications/Herdr Core.app/Contents/MacOS/herdr-core" "$out/bin/herdr-core"
  ''
  + lib.optionalString stdenvNoCC.hostPlatform.isLinux ''
    install -m755 herdr-core "$out/bin/herdr-core"
  ''
  + ''
    ln -s herdr-core "$out/bin/hc"
    cp LICENSE THIRD-PARTY-NOTICES "$out/share/doc/herdr-core/"
    runHook postInstall
  '';

  doInstallCheck = true;
  installCheckPhase = ''
    runHook preInstallCheck
  ''
  + lib.optionalString stdenvNoCC.hostPlatform.isLinux ''
    test "$("$out/bin/herdr-core" --version)" = "hc ${version}"
    test "$("$out/bin/hc" --version)" = "hc ${version}"
  ''
  + lib.optionalString stdenvNoCC.hostPlatform.isDarwin ''
    /usr/bin/codesign --verify --strict \
      -R '=identifier "dev.herdr.core" and anchor apple generic and certificate leaf[subject.OU] = "2W9652Q7QR"' \
      "$out/Applications/Herdr Core.app"
    # Launching the output bundle attaches a macOS MACL that prevents Nix
    # from finalizing the store path. Execute its identical bytes outside it.
    cp -L "$out/bin/herdr-core" "$TMPDIR/herdr-core-check"
    cmp "$out/bin/hc" "$TMPDIR/herdr-core-check"
    test "$("$TMPDIR/herdr-core-check" --version)" = "hc ${version}"
  ''
  + ''
    runHook postInstallCheck
  '';

  meta = {
    description = "Persistent terminal runtime for coding agents";
    homepage = "https://github.com/herdrdev/herdr-core";
    license = lib.licenses.asl20;
    platforms = builtins.attrNames platforms;
    mainProgram = "herdr-core";
  };
}
