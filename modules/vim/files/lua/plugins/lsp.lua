return {
  {
    "neovim/nvim-lspconfig",
    opts = function(_, opts)
      local keys = {
        { "gd", function() Snacks.picker.lsp_definitions() end, desc = "Goto Definition" },
        { "gr", function() Snacks.picker.lsp_references() end, desc = "References" },
        { "gb", "<C-o>", desc = "Jump Back" },
      }
      opts.servers["*"].keys = vim.list_extend(opts.servers["*"].keys, keys)
      for _, name in ipairs({
        "nixd", "gopls", "rust_analyzer", "ts_ls", "html", "cssls", "jsonls",
        "lua_ls", "bashls", "terraformls", "yamlls", "marksman", "taplo", "ty",
      }) do
        opts.servers[name] = vim.tbl_deep_extend("force", opts.servers[name] or {}, { mason = false })
      end
      opts.servers.rust_analyzer.settings = { ["rust-analyzer"] = { files = { watcher = "client" } } }
      opts.diagnostics.virtual_lines = false
      opts.diagnostics.virtual_text = {
        current_line = true, spacing = 2, prefix = "●", virt_text_pos = "eol",
      }
      opts.diagnostics.signs = { text = {}, numhl = {} }
      for name, severity in pairs(vim.diagnostic.severity) do
        if type(name) == "string" then
          opts.diagnostics.signs.text[severity] = "●"
          opts.diagnostics.signs.numhl[severity] = "Diagnostic" .. name:sub(1, 1) .. name:sub(2):lower()
        end
      end
    end,
  },
}
