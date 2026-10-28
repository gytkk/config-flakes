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
  assert(vim.o.autowrite and vim.o.relativenumber and vim.o.list and vim.o.cursorline)
  assert(vim.opt.listchars:get().trail == " ", "Trailing spaces must display as blanks")
  assert(vim.go.wrap == false and vim.wo.wrap and not vim.wo.spell)
  local markdown = vim.api.nvim_get_current_buf()
  vim.wo.wrap = false
  vim.wo.linebreak = false
  vim.cmd("enew")
  vim.api.nvim_win_set_buf(0, markdown)
  assert(vim.wo.wrap and vim.wo.linebreak, "Returning to Markdown must restore display wrapping")
  assert(vim.go.conceallevel == 2 and vim.o.laststatus == 3 and vim.o.updatetime == 200)
  assert(vim.o.foldlevel == 99 and vim.o.foldmethod ~= "manual")
  assert(vim.o.foldlevelstart == 99)
  assert(vim.bo.formatexpr == "v:lua.LazyVim.format.formatexpr()")
  assert(vim.g.mapleader == " " and vim.g.maplocalleader == "\\")
  assert(not Snacks.config.scroll.enabled)
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
  assert(lsp.folds.enabled and Plugin.values(plugins["nvim-treesitter"], "opts").folds.enable)
  assert(not vim.lsp.is_enabled("pyright") and not vim.lsp.is_enabled("vtsls"))
  local diagnostics = vim.diagnostic.config()
  assert(diagnostics.virtual_text.current_line and diagnostics.virtual_lines == false)
  local conform = Plugin.values(plugins["conform.nvim"], "opts")
  assert(conform.format_on_save == nil and conform.format_after_save == nil)
  assert(vim.deep_equal(conform.formatters_by_ft.typescript, { "biome" }))
  assert(conform.default_format_opts.timeout_ms == 1000)

  require("lazy").load({ plugins = { "grug-far.nvim", "trouble.nvim", "lualine.nvim" } })
  for lhs, desc in pairs({
    [" sR"] = "Resume", [" sr"] = "Search and Replace", [" s/"] = "Search History",
    [" sb"] = "Buffer Lines", [" fb"] = "Buffers", [" ,"] = "Buffers",
    [" cS"] = "LSP references/definitions/... (Trouble)",
    [" ud"] = "Toggle Diagnostics", [" uh"] = "Toggle Inlay Hints",
    [" ul"] = "Toggle Line Numbers", [" uS"] = "Toggle Smooth Scroll", [" uw"] = "Toggle Wrap",
  }) do
    assert(vim.fn.maparg(lhs, "n", false, true).desc == desc, "Unexpected mapping: " .. lhs)
  end
  for _, lhs in ipairs({ " b", " rn", " gh", " hs", " hr", " hp", " hb", "gi" }) do
    assert(vim.tbl_isempty(vim.fn.maparg(lhs, "n", false, true)), "Legacy mapping must be absent: " .. lhs)
  end
  local lspKeys = require("lazy.core.handler.keys").resolve(lsp.servers["*"].keys)
  for _, lhs in ipairs({ "gI", " cr", " cl" }) do
    assert(lspKeys[lhs] and lspKeys[lhs].rhs, "Missing default LSP mapping: " .. lhs)
  end
  assert(vim.fn.maparg(" cl", "n", false, true).desc == "Lsp Info")
  assert(vim.wait(1000, function()
    return vim.fn.maparg(" ghs", "n", false, true).desc == "Stage Hunk"
  end), "Default Git mappings must attach to the tracked file")
  assert(vim.fn.maparg(" ghs", "x", false, true).desc == "Stage Hunk")
  assert(vim.fn.maparg(" ghp", "n", false, true).desc == "Preview Hunk Inline")
  local lualine = Plugin.values(plugins["lualine.nvim"], "opts")
  assert(lualine.sections.lualine_x[1] == require("minuet.lualine"))
  assert(#lualine.sections.lualine_c >= 4 and #lualine.sections.lualine_x > 1)
  assert(lualine.options.globalstatus)

  local cwd = vim.fn.getcwd()
  local root = LazyVim.root()
  vim.api.nvim_set_current_dir(cwd .. "/modules/vim")
  assert(LazyVim.root() == root and LazyVim.root() ~= vim.fn.getcwd())
  local pickerOpen = LazyVim.pick.picker.open
  local picked
  LazyVim.pick.picker.open = function(source, opts)
    picked = { source, opts.cwd or vim.fn.getcwd() }
  end
  for _, case in ipairs({
    { " ff", "files", root }, { " fF", "files", vim.fn.getcwd() },
    { " /", "grep", root }, { " sg", "grep", root }, { " sG", "grep", vim.fn.getcwd() },
    { " sw", "grep_word", root }, { " sW", "grep_word", vim.fn.getcwd() },
  }) do
    vim.fn.maparg(case[1], "n", false, true).callback()
    assert(vim.deep_equal(picked, { case[2], case[3] }), "Incorrect search scope: " .. case[1])
  end
  LazyVim.pick.picker.open = pickerOpen
  vim.api.nvim_set_current_dir(cwd)

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
    assert(vim.wo.wrap == (ft == "markdown"), "Unexpected default wrapping: " .. ft)
    vim.api.nvim_exec_autocmds("BufWritePre", { buffer = buf })
    assert(formatted == (ft == "lua" and 1 or 0), "Save formatting policy failed: " .. vim.inspect({
      filetype = ft, count = formatted, global = vim.g.autoformat, buffer = vim.b[buf].autoformat,
      hooks = #vim.api.nvim_get_autocmds({ event = "BufWritePre" }),
    }))
  end
  for _, ft in ipairs({ "markdown", "yaml" }) do
    vim.bo.filetype = ft
    assert(vim.b.autoformat == false)
    vim.fn.maparg(" cf", "n", false, true).callback()
  end
  assert(formatted == 3, "Default manual formatting must still work for Markdown and YAML")
  vim.bo.filetype = "lua"
  assert(vim.b.autoformat == nil, "Changing from Markdown to Lua must restore global formatting policy")
  io.stdout:write("PASS: LazyVim startup, Markdown/MDX, Marksman, theme, Nix LSP ownership, default options/keymaps, root/cwd search scopes, Minuet statusline, diagnostics, save/manual formatting\n")
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
