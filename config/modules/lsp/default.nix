{
  pkgs,
  lib,
  config,
  ...
}:

let
  cfg = config.modules.lsp;
in
{
  options.modules.lsp = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Enable LSP server packages";
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = with pkgs; [
      # Nix
      nixd

      # Go
      gopls

      # TypeScript
      typescript-language-server

      # Web (HTML, CSS, JSON)
      vscode-langservers-extracted

      # Lua
      lua-language-server

      # Shell
      bash-language-server

      # Terraform
      terraform-ls

      # Python
      ty
      pyright

      # YAML
      yaml-language-server

      # Markdown
      marksman

      # TOML
      taplo

      # Rust: install rust-analyzer in the active rustup toolchain
    ];
  };
}
