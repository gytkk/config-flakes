return {
  {
    "stevearc/conform.nvim",
    opts = function(_, opts)
      opts.default_format_opts = { timeout_ms = 1000, lsp_format = "fallback" }
      opts.formatters = opts.formatters or {}
      opts.formatters.lukeed_tidy = {
        command = vim.fn.stdpath("data") .. "/lukeed-tidy/tidy",
        args = function(_, ctx)
          return { "up", "--stdin=" .. ctx.filename, "--lang", vim.bo[ctx.buf].filetype }
        end,
      }
      opts.formatters_by_ft = {
        nix = { "nixfmt" },
        go = { "gofmt" },
        rust = { "rustfmt" },
        javascript = { "biome" },
        javascriptreact = { "biome" },
        typescript = { "biome" },
        typescriptreact = { "biome" },
        json = { "lukeed_tidy" },
        jsonc = { "lukeed_tidy" },
        css = { "biome" },
        yaml = { "prettier" },
        markdown = { "prettier" },
        html = { "prettier" },
        python = { "ruff_format" },
      }
    end,
  },
  {
    "iamcco/markdown-preview.nvim",
    cmd = { "MarkdownPreview", "MarkdownPreviewStop", "MarkdownPreviewToggle" },
    ft = { "markdown" },
    build = "cd app && npx --yes yarn install --frozen-lockfile",
    init = function()
      vim.g.mkdp_auto_start = 0
      vim.api.nvim_create_user_command("MarkdownViewer", function()
        vim.cmd("MarkdownPreview")
      end, { desc = "Open Markdown preview in the default browser" })
    end,
  },
}
