let
  flake =
    if builtins.hasAttr "getFlake" builtins then
      (builtins.getAttr "getFlake" builtins) (toString ../../..)
    else
      throw "Pi module checks require Nix with flakes enabled";
  pkgs = import flake.inputs.nixpkgs { system = builtins.currentSystem; };
  lib = pkgs.lib.extend (_: _: import "${flake.inputs.home-manager}/lib" { lib = pkgs.lib; });
  defaultSettings =
    (import ../default.nix {
      inherit lib pkgs;
      config = { };
      flakeDirectory = "/checkout";
      themeExports = { };
    }).options.modules.pi.settingsFile.default;
  devsistersSettings =
    (import ../../../systems/base/devsisters/home.nix {
      inherit lib;
      config = { };
      username = "test";
      homeDirectory = "/home/test";
    }).modules.pi.settingsFile;
  evaluate =
    {
      herdrEnabled ? true,
      recorderEnabled ? false,
      recordPi ? true,
      pi ? { },
    }:
    let
      module = import ../default.nix {
        inherit lib;
        pkgs = pkgs // {
          # Materialize only settings during evaluation, without building packages.
          writeText = builtins.toFile;
          fetchzip = args: "/fixtures/${args.name}";
          herdr.src = "/herdr";
          pi.name = "pi";
        };
        config = {
          modules.pi = {
            enable = true;
            settingsFile = defaultSettings;
            installPackage = true;
            launchEnvironment = {
              enable = false;
              pathHead = [ ];
            };
          }
          // pi;
          modules.herdr.enable = herdrEnabled;
          modules.agentSessionRecord = {
            enable = recorderEnabled;
            agents.pi.enable = recordPi;
          };
          lib.file.mkOutOfStoreSymlink = path: path;
        };
        flakeDirectory = "/checkout";
        themeExports.file = app: name: "/themes/${app}/${name}";
      };
      config = module.config.content;
      settingsPath = builtins.head (
        builtins.match ".*run [^ ]+ ([^ ]+) .*" config.home.activation.piSettings.data
      );
    in
    {
      settings = builtins.fromJSON (builtins.readFile settingsPath);
      interactiveShellSource = config.home.file.".pi/agent/interactive-shell.json".source;
      sessionVariables = config.home.sessionVariables;
      packages = map (package: package.name) config.home.packages;
      lensConfigSource = config.home.file.".pi-lens/config.json".source;
      legacyLspConfig = builtins.hasAttr ".pi/agent/lsp.json" config.home.file;
      extensionSources = lib.mapAttrs (_: file: file.source) (
        lib.filterAttrs (name: _: lib.hasPrefix ".pi/agent/extensions/" name) config.home.file
      );
      activation = {
        inherit (config.home.activation.piSettings) before after;
      };
    };
in
{
  enabled = evaluate { };
  recording = evaluate { recorderEnabled = true; };
  piRecordingOff = evaluate {
    recorderEnabled = true;
    recordPi = false;
  };
  disabled = evaluate { herdrEnabled = false; };
  devsisters = evaluate { pi.settingsFile = devsistersSettings; };
  # A jari worker: its own pinned Pi, started with jari-worker.service's fixed environment.
  jari = evaluate {
    herdrEnabled = false;
    pi = {
      enable = true;
      settingsFile =
        (import ../../../systems/base/jari/home.nix {
          inherit lib pkgs;
          config = { };
        }).modules.pi.settingsFile;
      installPackage = false;
      launchEnvironment = {
        enable = true;
        pathHead = [
          "/opt/jari/bin"
          "/profile/bin"
        ];
      };
    };
  };
}
