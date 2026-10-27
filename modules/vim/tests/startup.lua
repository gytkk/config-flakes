-- Keep test buffers out of the user's saved session.
require("persistence").stop()

-- lazy.nvim starts VeryLazy after UIEnter; headless checks must supply that event.
vim.api.nvim_exec_autocmds("UIEnter", { modeline = false })

local function check()
  assert(LazyVim.config.version == "16.0.1")
  assert(not LazyVim.has("mason.nvim") and not LazyVim.has("mason-lspconfig.nvim"))
  assert(not LazyVim.has("bufferline.nvim"))
  assert(not Snacks.config.dashboard.enabled)
  assert(vim.g.colors_name and vim.g.colors_name ~= "tokyonight")
  assert(vim.bo.filetype == "markdown", "Open a Markdown or MDX fixture directly")
  assert(vim.b.autoformat == false)
  assert(require("lazy.core.config").plugins["render-markdown.nvim"]._.loaded)
  assert(vim.wait(5000, function()
    return #vim.lsp.get_clients({ bufnr = 0, name = "marksman" }) == 1
  end), "Marksman must attach to the first Markdown/MDX file")
  assert(vim.treesitter.get_parser(0, "markdown"))
  local Plugin = require("lazy.core.plugin")
  local plugins = require("lazy.core.config").plugins
  local lsp = Plugin.values(plugins["nvim-lspconfig"], "opts")
  for _, server in ipairs({
    "nixd", "gopls", "rust_analyzer", "ts_ls", "html", "cssls", "jsonls",
    "lua_ls", "bashls", "terraformls", "yamlls", "marksman", "taplo", "ty",
  }) do
    assert(lsp.servers[server].mason == false and vim.lsp.is_enabled(server), server)
  end
  assert(lsp.servers.rust_analyzer.settings["rust-analyzer"].files.watcher == "client")
  assert(not vim.lsp.is_enabled("pyright") and not vim.lsp.is_enabled("vtsls"))
  local diagnostics = vim.diagnostic.config()
  assert(diagnostics.virtual_text.current_line and diagnostics.virtual_lines == false)
  local conform = Plugin.values(plugins["conform.nvim"], "opts")
  assert(conform.format_on_save == nil and conform.format_after_save == nil)
  assert(vim.deep_equal(conform.formatters_by_ft.typescript, { "biome" }))
  assert(conform.default_format_opts.timeout_ms == 1000)

  local formatted = 0
  LazyVim.format.register({
    name = "offline-test", primary = true, priority = 10000,
    sources = function() return { "offline-test" } end,
    format = function() formatted = formatted + 1 end,
  })
  for _, ft in ipairs({ "markdown", "yaml", "lua" }) do
    local buf = vim.api.nvim_create_buf(true, false)
    vim.api.nvim_set_current_buf(buf)
    vim.bo[buf].filetype = ft
    vim.api.nvim_exec_autocmds("BufWritePre", { buffer = buf })
    assert(formatted == (ft == "lua" and 1 or 0), "Save formatting policy failed: " .. vim.inspect({
      filetype = ft, count = formatted, global = vim.g.autoformat, buffer = vim.b[buf].autoformat,
      hooks = #vim.api.nvim_get_autocmds({ event = "BufWritePre" }),
    }))
  end
  local format = require("conform").format
  require("conform").format = function() formatted = formatted + 1 end
  vim.bo.filetype = "markdown"
  local keys = require("lazy.core.handler.keys").resolve(Plugin.values(plugins["conform.nvim"], "keys"))
  for _, key in pairs(keys) do
    if key.lhs == "<leader>cf" then key.rhs() end
  end
  require("conform").format = format
  assert(formatted == 2, "Manual formatting must still work for Markdown")
  vim.bo.filetype = "lua"
  assert(vim.b.autoformat == nil, "Changing from Markdown to Lua must restore global formatting policy")
  assert(vim.fn.maparg(" sr", "n") ~= "" or vim.fn.maparg(" sr", "n", false, true).callback)
  io.stdout:write("PASS: LazyVim startup, startup Markdown/MDX, Marksman, theme, Nix LSP ownership, diagnostics, save/manual formatting\n")
end

vim.defer_fn(function()
  local ok, err = xpcall(check, debug.traceback)
  if not ok then
    vim.api.nvim_err_writeln(err)
    vim.cmd("cquit 1")
  else
    vim.cmd("qa!")
  end
end, 1000)
