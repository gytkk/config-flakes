return {
  {
    "LazyVim/LazyVim",
    opts = { colorscheme = require("config.theme") },
  },
  { "mason-org/mason.nvim", enabled = false },
  { "mason-org/mason-lspconfig.nvim", enabled = false },
  { "akinsho/bufferline.nvim", enabled = false },
  { "folke/tokyonight.nvim", enabled = false },
  { "catppuccin/nvim", enabled = false },
  {
    "folke/persistence.nvim",
    opts = { need = 1, branch = true },
    init = function()
      -- Register VimEnter before it fires; config/autocmds.lua can load after it.
      if #vim.api.nvim_list_uis() > 0 then
        require("config.session").setup()
      end
    end,
  },
  {
    "nvim-treesitter/nvim-treesitter",
    opts = {
      folds = { enable = false },
      ensure_installed = {
        "go", "gomod", "gosum", "rust", "nix", "hcl", "terraform", "dockerfile", "css",
      },
    },
  },
}
