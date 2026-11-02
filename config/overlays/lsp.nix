{ nixpkgs, system }:
let
  pkgs = import nixpkgs {
    inherit system;
    overlays = [
      (final: prev: {
        # nixd and its libraries must use the same source release.
        nixf = prev.nixf.overrideAttrs (_: {
          version = "2.9.4";
          src = final.fetchFromGitHub {
            owner = "nix-community";
            repo = "nixd";
            tag = "2.9.4";
            hash = "sha256-iBoajmSfgWQ7dJGrL8I2qgK+TsS1lDTnGSTo/qwsfbc=";
          };
        });
        nixt = prev.nixt.overrideAttrs (_: {
          inherit (final.nixf) version src;
        });
        nixd = prev.nixd.overrideAttrs (_: {
          inherit (final.nixf) version src;
        });
        vscodium = prev.vscodium.overrideAttrs (_: {
          version = "1.143.17232";
          src = final.fetchurl {
            url = "https://github.com/VSCodium/vscodium/releases/download/1.143.17232/VSCodium-${
              if final.stdenv.hostPlatform.isDarwin then
                "darwin-arm64-1.143.17232.zip"
              else
                "linux-x64-1.143.17232.tar.gz"
            }";
            hash =
              {
                aarch64-darwin = "sha256-rej14Ov0fl6p4keIRPFQxo5D3vGStvi+BdAynSXsCRc=";
                x86_64-linux = "sha256-yhiVqgQL2RoJxwnXnqx4YtjBqgI3/cJbUD3uuLzYfv0=";
              }
              .${system};
          };
        });
      })
    ];
  };
in
{
  inherit (pkgs)
    ty
    gopls
    terraform-ls
    nixd
    yaml-language-server
    vscode-langservers-extracted
    ;
}
