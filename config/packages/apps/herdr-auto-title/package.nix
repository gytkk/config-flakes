{
  lib,
  buildGoModule,
  fetchzip,
}:

buildGoModule rec {
  pname = "herdr-auto-title";
  version = "0.20.0";

  src = fetchzip {
    url = "https://github.com/kryptamine/herdr-auto-title/archive/refs/tags/v${version}.tar.gz";
    hash = "sha256-1Ebc40+fU0HtvuOmFrV+sAU+5GGMoJhWDj0++93fWg4=";
  };

  vendorHash = "sha256-BN/q/5o9CiklREYZloLZU2rLKUIW6Eq6Yy1Wr9vK3hU=";
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
