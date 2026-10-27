local legacy = require("config.legacy")
local ui = legacy("plugins.ui")
local editing = legacy("plugins.editing")
local snacks = legacy("plugins.snacks")
snacks.opts.dashboard = { enabled = false }
local session = legacy("session")

return {
  {
    "LazyVim/LazyVim",
    opts = { colorscheme = function() legacy("theme").config() end },
  },
  { "mason-org/mason.nvim", enabled = false },
  { "mason-org/mason-lspconfig.nvim", enabled = false },
  { "akinsho/bufferline.nvim", enabled = false },
  { "folke/tokyonight.nvim", enabled = false },
  { "catppuccin/nvim", enabled = false },
  snacks,
  ui.noice,
  ui.flash,
  ui.whichkey,
  ui.lualine,
  ui.trouble,
  legacy("plugins.git"),
  {
    "folke/persistence.nvim",
    opts = session.plugin.opts,
    init = function()
      -- Register VimEnter before it fires; config/autocmds.lua can load after it.
      if #vim.api.nvim_list_uis() > 0 then
        session.setup()
      end
    end,
  },
  {
    "stevearc/conform.nvim",
    opts = function(_, opts)
      opts.formatters_by_ft = editing.conform.opts.formatters_by_ft
      opts.default_format_opts = { timeout_ms = 1000, lsp_format = "fallback" }
    end,
    keys = editing.conform.keys,
  },
  editing.render_markdown,
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
