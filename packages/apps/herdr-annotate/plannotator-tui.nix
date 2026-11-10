{
  lib,
  rustPlatform,
  fetchzip,
  sources ? builtins.fromJSON (builtins.readFile ./sources.json),
}:

rustPlatform.buildRustPackage {
  pname = "plannotator-tui";
  version = sources.plannotatorTui.version;

  src = fetchzip {
    url = "https://github.com/plannotator/plannotator-tui/archive/refs/tags/v${sources.plannotatorTui.version}.tar.gz";
    hash = sources.plannotatorTui.sourceHash;
  };

  cargoHash = sources.plannotatorTui.cargoHash;

  preCheck = ''
    export PLANNOTATOR_DATA_DIR="$TMPDIR/plannotator-test-data"
    # Nix's cc/ld response files misquote the terminal tests' Unicode fixture paths.
    export NIX_CC_USE_RESPONSE_FILE=0
    export NIX_LD_USE_RESPONSE_FILE=0
  '';

  meta = {
    description = "Markdown review TUI";
    homepage = "https://github.com/plannotator/plannotator-tui";
    license = lib.licenses.mit;
    mainProgram = "plannotator-tui";
    platforms = lib.platforms.darwin ++ lib.platforms.linux;
  };
}
