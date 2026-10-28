vim.api.nvim_del_augroup_by_name("lazyvim_checktime")

vim.api.nvim_create_autocmd({ "FileType", "BufWinEnter" }, {
  callback = function(event)
    if vim.bo[event.buf].filetype ~= "markdown" then return end
    vim.opt_local.wrap = true
    vim.opt_local.linebreak = true
    vim.opt_local.spell = false
  end,
})

local function refreshReloadedBuffer(buf)
  if not vim.api.nvim_buf_is_valid(buf) or vim.bo[buf].buftype ~= "" then return end

  local has_treesitter = pcall(vim.treesitter.get_parser, buf)
  if has_treesitter then
    pcall(vim.treesitter.stop, buf)
    pcall(vim.treesitter.start, buf)
  end

  vim.bo[buf].syntax = "ON"
  pcall(vim.api.nvim_buf_call, buf, function()
    vim.cmd("silent! syntax sync fromstart")
  end)

  vim.schedule(function()
    if not vim.api.nvim_buf_is_valid(buf) then return end

    -- Drop extmarks anchored to the pre-reload text and request fresh hints.
    if vim.lsp.inlay_hint.is_enabled({ bufnr = buf }) then
      vim.lsp.inlay_hint.enable(false, { bufnr = buf })
      vim.lsp.inlay_hint.enable(true, { bufnr = buf })
    end

    if vim.api.nvim_get_current_buf() == buf then
      vim.cmd("redraw!")
    end
  end)
end

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
  callback = function()
    if vim.fn.mode() == "c" then return end

    local buf = vim.api.nvim_get_current_buf()
    if vim.bo[buf].buftype ~= "" or vim.bo[buf].modified then return end

    vim.cmd("checktime")
  end,
})

vim.api.nvim_create_autocmd("FileChangedShellPost", {
  callback = function(event)
    refreshReloadedBuffer(event.buf)
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
