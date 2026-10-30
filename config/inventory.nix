{
  "pylv-sepia" = {
    kind = "nixos";
    system = "x86_64-linux";
    username = "gytkk";
    homeDirectory = "/home/gytkk";
    profile = "pylv";
    homeModules = [ ./base/pylv/sepia.nix ];
  };

  "pylv-onyx" = {
    kind = "nixos";
    system = "x86_64-linux";
    username = "gytkk";
    homeDirectory = "/home/gytkk";
    profile = "pylv";
    homeModules = [ ./base/pylv/onyx.nix ];
  };

  "pylv-denim" = {
    kind = "home-only";
    system = "x86_64-linux";
    username = "gytkk";
    homeDirectory = "/home/gytkk";
    profile = "pylv";
    isWSL = true;
    homeModules = [ ./base/pylv/denim.nix ];
  };

  "devsisters-macbook" = {
    kind = "home-only";
    system = "aarch64-darwin";
    username = "gyutak";
    homeDirectory = "/Users/gyutak";
    profile = "devsisters";
    homeModules = [ ./base/devsisters/session-mining.nix ];
  };

  "devsisters-macstudio" = {
    kind = "home-only";
    system = "aarch64-darwin";
    username = "gyutak";
    homeDirectory = "/Users/gyutak";
    profile = "devsisters";
    homeModules = [ ./base/devsisters/session-mining.nix ];
  };
}
