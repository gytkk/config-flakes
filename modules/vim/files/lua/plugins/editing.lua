return {
  {
    "stevearc/conform.nvim",
    opts = function(_, opts)
      opts.default_format_opts = { timeout_ms = 1000, lsp_format = "fallback" }
      opts.formatters_by_ft = {
        nix = { "nixfmt" },
        go = { "gofmt" },
        rust = { "rustfmt" },
        javascript = { "biome" },
        javascriptreact = { "biome" },
        typescript = { "biome" },
        typescriptreact = { "biome" },
        json = { "biome" },
        jsonc = { "biome" },
        css = { "biome" },
        yaml = { "prettier" },
        markdown = { "prettier" },
        html = { "prettier" },
        python = { "ruff_format" },
      }
    end,
  },
  {
    "MeanderingProgrammer/render-markdown.nvim",
    ft = { "markdown" },
    dependencies = {
      "nvim-treesitter/nvim-treesitter",
      "nvim-tree/nvim-web-devicons",
    },
    opts = {
      change_events = { "FileChangedShellPost" },
    },
  },
}
