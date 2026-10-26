{
  lib,
  buildGoModule,
  fetchzip,
}:

buildGoModule rec {
  pname = "herdr-auto-title";
  version = "0.16.0";

  src = fetchzip {
    url = "https://github.com/kryptamine/herdr-auto-title/archive/refs/tags/v${version}.tar.gz";
    hash = "sha256-DcfaMUmaPRvZ8D/TCy/veuOjWl3VNvcsrTbh6XAooNE=";
  };

  vendorHash = "sha256-QxFp1b7pf7bn3Hh0hyaj8ke5Z61N+WwjhHt3pFiapTs=";
  subPackages = [ "cmd/herdr-auto-title" ];
  env.CGO_ENABLED = 0;

  checkPhase = ''
    runHook preCheck
    go test ./...
    runHook postCheck
  '';

  postInstall = ''
    pluginRoot="$out/share/herdr/plugins/auto-title"
    mkdir -p "$pluginRoot"
    sed '/^\[\[build\]\]$/,/^$/d' herdr-plugin.toml > "$pluginRoot/herdr-plugin.toml"
    ln -s "$out/bin/herdr-auto-title" "$pluginRoot/herdr-auto-title"
  '';

  meta = {
    description = "Herdr plugin for automatic tab and pane titles";
    homepage = "https://github.com/kryptamine/herdr-auto-title";
    license = lib.licenses.mit;
    platforms = lib.platforms.darwin ++ lib.platforms.linux;
    mainProgram = "herdr-auto-title";
  };
}
