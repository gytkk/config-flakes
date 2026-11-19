vim.api.nvim_del_augroup_by_name("lazyvim_checktime")

vim.api.nvim_create_autocmd({ "FileType", "BufWinEnter" }, {
  callback = function(event)
    local ft = vim.bo[event.buf].filetype
    if ft ~= "markdown" and ft ~= "yaml" then return end
    vim.opt_local.wrap = true
    vim.opt_local.linebreak = true
    if ft == "markdown" then vim.opt_local.spell = false end
  end,
})

-- Neovim invalidates Tree-sitter and syntax state on reload without restarting either.
-- Its LSP close/open cycle also clears and refreshes hints for syncing clients.
vim.api.nvim_create_autocmd("FileChangedShellPost", {
  desc = "Refresh inlay hints for clients without document open/close sync",
  callback = function(event)
    local buf = event.buf
    vim.schedule(function()
      if not vim.api.nvim_buf_is_loaded(buf) or vim.bo[buf].buftype ~= "" then return end
      local opts = { bufnr = buf }
      if not vim.lsp.inlay_hint.is_enabled(opts) then return end
      for _, client in ipairs(vim.lsp.get_clients({ bufnr = buf, method = "textDocument/inlayHint" })) do
        if not client:supports_method("textDocument/didOpen", buf)
            or not client:supports_method("textDocument/didClose", buf) then
          vim.lsp.inlay_hint.enable(false, opts)
          vim.lsp.inlay_hint.enable(true, opts)
          break
        end
      end
    end)
  end,
})

vim.api.nvim_create_autocmd({ "InsertEnter", "TextChanged", "BufLeave", "FileChangedShellPost" }, {
  desc = "Clear completed search highlights and the Noice search count",
  callback = function()
    -- Neovim restores search highlighting after an autocmd, so clear it afterward.
    vim.schedule(function() vim.cmd.nohlsearch() end)
  end,
})

vim.api.nvim_create_autocmd("BufEnter", {
  callback = function()
    vim.schedule(function()
      local win = vim.api.nvim_get_current_win()
      if not vim.api.nvim_win_is_valid(win) then return end
      local buf = vim.api.nvim_win_get_buf(win)
      if vim.bo[buf].buftype == "" and vim.bo[buf].buflisted then
        local whl = vim.wo[win].winhighlight
        if whl and whl:find("Snacks") then
          vim.wo[win].winhighlight = ""
        end
      end
    end)
  end,
})

vim.api.nvim_create_autocmd({ "FocusGained", "BufEnter", "CursorHold", "CursorHoldI", "TermClose", "TermLeave" }, {
  nested = true, -- Let reload hooks run when checktime reloads synchronously.
  callback = function(event)
    if vim.fn.mode() == "c" then return end

    local buf = vim.api.nvim_get_current_buf()
    if vim.bo[buf].buftype ~= "" or vim.bo[buf].modified then return end

    if event.event == "FocusGained" then
      -- An unscoped :checktime defers itself inside autocmds.
      vim.schedule(vim.cmd.checktime)
    else
      vim.cmd.checktime({ count = buf })
    end
  end,
})

local function formatPolicy(buf)
  local ft = vim.bo[buf].filetype
  if ft == "markdown" or ft == "yaml" then
    vim.b[buf].autoformat = false
  else
    vim.b[buf].autoformat = nil
  end
end
vim.api.nvim_create_autocmd("FileType", {
  callback = function(event)
    formatPolicy(event.buf)
  end,
})
for _, buf in ipairs(vim.api.nvim_list_bufs()) do
  formatPolicy(buf)
end
