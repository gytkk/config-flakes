{
  lib,
  stdenv,
  fetchurl,
  fetchzip,
  autoPatchelfHook,
  bash,
  callPackage,
}:

let
  version = "0.9.8";
  sources = builtins.fromJSON (builtins.readFile ./sources.json);

  platformMap = {
    "aarch64-darwin" = "aarch64-apple-darwin";
    "x86_64-darwin" = "x86_64-apple-darwin";
    "x86_64-linux" = "x86_64-unknown-linux-gnu";
    "aarch64-linux" = "aarch64-unknown-linux-gnu";
  };

  target =
    platformMap.${stdenv.hostPlatform.system}
      or (throw "Unsupported system: ${stdenv.hostPlatform.system}");

  src = fetchzip {
    url = "https://github.com/plannotator/herdr-annotate/archive/${sources.revision}.tar.gz";
    hash = sources.sourceHash;
  };

  herdrAnnotate = fetchurl {
    url = "https://github.com/plannotator/herdr-annotate/releases/download/rust-lite-v${sources.herdrAnnotate.version}/herdr-annotate-${target}";
    hash = sources.herdrAnnotate.hashes.${target};
  };

  plannotatorTui = callPackage ./plannotator-tui.nix { inherit sources; };
in
stdenv.mkDerivation {
  pname = "herdr-annotate";
  inherit version src;

  nativeBuildInputs = lib.optionals stdenv.hostPlatform.isLinux [ autoPatchelfHook ];
  buildInputs = lib.optionals stdenv.hostPlatform.isLinux [ stdenv.cc.cc.lib ];

  dontBuild = true;
  dontStrip = true;

  installPhase = ''
    runHook preInstall

    pluginRoot="$out/share/herdr/plugins/annotate"
    mkdir -p "$pluginRoot/bin" "$pluginRoot/scripts" "$out/bin"

    sed '/^\[\[build\]\]$/,/^$/d' herdr-plugin.toml > "$pluginRoot/herdr-plugin.toml"
    substituteInPlace "$pluginRoot/herdr-plugin.toml" \
      --replace-fail 'command = ["sh", "-c", "exec bash ' 'command = ["${bash}/bin/bash", "-c", "exec ${bash}/bin/bash '

    install -Dm755 ${herdrAnnotate} "$pluginRoot/bin/herdr-annotate.exe"
    install -Dm755 ${plannotatorTui}/bin/plannotator-tui "$pluginRoot/bin/plannotator-tui.exe"
    install -Dm755 scripts/plannotator-tui.sh "$pluginRoot/scripts/plannotator-tui.sh"
    substituteInPlace "$pluginRoot/scripts/plannotator-tui.sh" \
      --replace-fail '#!/usr/bin/env bash' '#!${bash}/bin/bash'

    ln -s "$pluginRoot/bin/herdr-annotate.exe" "$out/bin/herdr-annotate"

    runHook postInstall
  '';

  meta = {
    description = "Herdr plugin for terminal and document annotations";
    homepage = "https://github.com/plannotator/herdr-annotate";
    license = lib.licenses.mit;
    platforms = builtins.attrNames platformMap;
    mainProgram = "herdr-annotate";
  };
}
