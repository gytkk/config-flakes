{
  config,
  flakeDirectory,
  inputs,
  lib,
  osConfig ? null,
  pkgs,
  themeExports,
  ...
}:

let
  cfg = config.modules.vim;
  mkSymlink =
    path: config.lib.file.mkOutOfStoreSymlink "${flakeDirectory}/config/modules/vim/${path}";
  nvimThemeExports = config.lib.file.mkOutOfStoreSymlink (themeExports.mutableDir "nvim");
  openAIKeySecretName = "openai-api-key";
  usesSystemAgenix = osConfig != null;
  openAIKeySecretPath =
    if usesSystemAgenix then
      osConfig.age.secrets.${openAIKeySecretName}.path
    else
      config.age.secrets.${openAIKeySecretName}.path;
in
{
  options.modules.vim.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = "Enable Neovim module";
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ pkgs.tree-sitter ];

    xdg.configFile."nvim/lua/config".source = mkSymlink "files/lua/config";
    xdg.configFile."nvim/lua/plugins".source = mkSymlink "files/lua/plugins";
    xdg.configFile."nvim/themes".source = nvimThemeExports;

    programs.neovim = {
      enable = true;
      package = inputs.neovim-nightly-overlay.packages.${pkgs.stdenv.hostPlatform.system}.default;
      defaultEditor = true;
      viAlias = true;
      vimAlias = true;

      initLua = ''
        vim.g.nix_flakes_theme = ${builtins.toJSON config.modules.commonTheme}
        vim.g.openai_api_key_path = ${builtins.toJSON openAIKeySecretPath}
        vim.g.nix_flakes_lockfile = ${builtins.toJSON "${flakeDirectory}/config/modules/vim/files/lazy-lock.json"}
        require('config.lazy')
      '';
    };
  };
}
