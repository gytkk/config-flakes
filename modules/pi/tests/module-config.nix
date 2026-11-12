let
  flake = builtins.getFlake (toString ../../..);
  pkgs = import flake.inputs.nixpkgs { system = builtins.currentSystem; };
  lib = pkgs.lib.extend (_: _: import "${flake.inputs.home-manager}/lib" { lib = pkgs.lib; });
  evaluate =
    herdrEnabled:
    let
      module = import ../default.nix {
        inherit lib;
        pkgs = pkgs // {
          # Materialize only settings during evaluation, without building packages.
          writeText = builtins.toFile;
          herdr.src = "/herdr";
        };
        config = {
          modules.pi.enable = true;
          modules.herdr.enable = herdrEnabled;
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
      extensionSources = lib.mapAttrs (_: file: file.source) (
        lib.filterAttrs (name: _: lib.hasPrefix ".pi/agent/extensions/" name) config.home.file
      );
      activation = {
        inherit (config.home.activation.piSettings) before after;
      };
    };
in
{
  enabled = evaluate true;
  disabled = evaluate false;
}
