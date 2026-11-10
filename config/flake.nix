{
  description = "Home Manager and NixOS configuration";

  inputs = {
    # Nix 패키지 모음
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    nixpkgs-lsp.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    nixpkgs-25_05.url = "github:nixos/nixpkgs/nixos-25.05";

    # Neovim development builds
    neovim-nightly-overlay.url = "github:nix-community/neovim-nightly-overlay";

    # Home Manager
    home-manager = {
      url = "github:nix-community/home-manager/master";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # Disko - declarative disk partitioning
    disko = {
      url = "github:nix-community/disko";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # nix-zed-extensions - for Zed extension management
    nix-zed-extensions = {
      url = "github:DuskSystems/nix-zed-extensions";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-overlay.follows = "rust-overlay";
    };

    # rust-overlay - shared input for nix-zed-extensions
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # agenix - secrets management for NixOS
    agenix = {
      url = "github:ryantm/agenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # copyparty - file server
    copyparty = {
      url = "github:9001/copyparty";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # kc2aws - Keycloak OIDC→SAML CLI authenticator (Devsisters 전용, private repo → SSH)
    keycloak2aws = {
      url = "git+ssh://git@github.com/devsisters/keycloak2aws";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # wg-cli - Devsisters WireGuard VPN CLI (Devsisters 전용, private repo → SSH)
    devsisters-wg = {
      url = "git+ssh://git@github.com/devsisters/wg";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "gws/flake-utils";
    };

    # gws - Google Workspace CLI
    gws = {
      url = "github:googleworkspace/cli";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # niri - scrollable tiling Wayland compositor
    niri.url = "github:sodiboo/niri-flake";

    # DankMaterialShell - all-in-one desktop shell for niri
    dms = {
      url = "github:AvengeMedia/DankMaterialShell";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs: import ./outputs.nix inputs;
}
