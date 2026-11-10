{
  lib,
  stdenv,
  rustPlatform,
  callPackage,
  runCommand,
  git,
  pkg-config,
  cctools ? null,
  xcbuild ? null,
  zig_0_16 ? callPackage ./zig.nix { },
}:

let
  version = "0.9.5";
  # Cargo.lock and the Zig manifest must be readable during evaluation without IFD.
  src = builtins.fetchTarball {
    url = "https://github.com/herdrdev/herdr/archive/refs/tags/v${version}.tar.gz";
    sha256 = "sha256-N7e7oCKxeCoxek6UT8Lweq16rSsQQi34QMb7C2iNpm8=";
  };
  zigDeps = callPackage "${src}/vendor/libghostty-vt/build.zig.zon.nix" {
    name = "herdr-libghostty-vt-zig-cache";
    inherit zig_0_16;
    # Zig's system cache requires real directories rather than a symlink farm.
    linkFarm =
      name: entries:
      runCommand name { } ''
        mkdir -p $out
        ${lib.concatMapStringsSep "\n" (entry: ''
          cp -rL ${entry.path} $out/${entry.name}
        '') entries}
      '';
  };
in
rustPlatform.buildRustPackage {
  pname = "herdr";
  inherit version src;

  cargoLock = {
    lockFile = "${src}/Cargo.lock";
  };

  patches = import ./patches.nix;

  nativeBuildInputs = [
    git
    pkg-config
  ]
  ++ lib.optionals stdenv.hostPlatform.isDarwin [
    cctools
    xcbuild
  ];

  env = {
    ZIG = lib.getExe zig_0_16;
    LIBGHOSTTY_VT_ZIG_SYSTEM_DIR = zigDeps;
  };

  preBuild = ''
    export ZIG_GLOBAL_CACHE_DIR="$TMPDIR/zig-global-cache"
    export ZIG_LOCAL_CACHE_DIR="$TMPDIR/zig-local-cache"
  '';

  preCheck = ''
    export XDG_CONFIG_HOME="$TMPDIR/herdr-test-config"
    export XDG_STATE_HOME="$TMPDIR/herdr-test-state"
    mkdir -p "$XDG_CONFIG_HOME" "$XDG_STATE_HOME"
  '';

  # Plugin tests temporarily change process-wide configuration paths.
  dontUseCargoParallelTests = true;
  cargoTestFlags = [
    "--bin"
    "herdr"
  ];
  checkFlags = [
    "app::api::plugins"
    "detect::tests::identify_agent"
    "ui::sidebar::tokens::tests::separator_"
  ];

  meta = {
    description = "Terminal workspace manager";
    homepage = "https://herdr.dev";
    license = lib.licenses.asl20;
    platforms = [
      "aarch64-darwin"
      "x86_64-darwin"
      "aarch64-linux"
      "x86_64-linux"
    ];
    mainProgram = "herdr";
  };
}
