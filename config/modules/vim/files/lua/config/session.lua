local M = {}

local function hasCurrentSession(persistence_module)
  local session = persistence_module.current()
  if vim.fn.filereadable(session) ~= 0 then return true end

  session = persistence_module.current({ branch = false })
  return vim.fn.filereadable(session) ~= 0
end

local function isEmptyNormalBuffer(buf)
  if vim.bo[buf].buftype ~= "" or vim.bo[buf].modified then return false end
  if vim.api.nvim_buf_get_name(buf) ~= "" then return false end
  if vim.api.nvim_buf_line_count(buf) ~= 1 then return false end

  return vim.api.nvim_buf_get_lines(buf, 0, 1, false)[1] == ""
end

local function closeSessionBlankWindows()
  local normal_wins = {}
  local blank_wins = {}

  for _, win in ipairs(vim.api.nvim_list_wins()) do
    if vim.api.nvim_win_is_valid(win) and vim.api.nvim_win_get_config(win).relative == "" then
      normal_wins[#normal_wins + 1] = win

      local buf = vim.api.nvim_win_get_buf(win)
      if isEmptyNormalBuffer(buf) then
        blank_wins[#blank_wins + 1] = win
      end
    end
  end

  for _, win in ipairs(blank_wins) do
    if #normal_wins <= 1 then return end
    if vim.api.nvim_win_is_valid(win) then
      pcall(vim.api.nvim_win_close, win, false)
      normal_wins = vim.tbl_filter(function(normal_win)
        return vim.api.nvim_win_is_valid(normal_win)
      end, normal_wins)
    end
  end
end

local function openStartupExplorer()
  if #Snacks.picker.get({ source = "explorer", tab = false }) > 0 then return end

  local current_win = vim.api.nvim_get_current_win()
  Snacks.explorer({ cwd = vim.fn.getcwd() })

  vim.defer_fn(function()
    if vim.api.nvim_win_is_valid(current_win) and not vim.w[current_win].snacks_layout then
      vim.api.nvim_set_current_win(current_win)
      return
    end

    for _, win in ipairs(vim.api.nvim_list_wins()) do
      if vim.api.nvim_win_is_valid(win)
          and vim.api.nvim_win_get_config(win).relative == ""
          and not vim.w[win].snacks_layout
      then
        vim.api.nvim_set_current_win(win)
        return
      end
    end
  end, 50)
end

function M.setup()
  vim.api.nvim_create_autocmd("User", {
    pattern = "PersistenceLoadPost",
    callback = function()
      closeSessionBlankWindows()
      for _, win in ipairs(vim.api.nvim_list_wins()) do
        vim.wo[win].foldlevel = 99
      end
    end,
  })

  vim.api.nvim_create_autocmd("User", {
    pattern = "VeryLazy",
    once = true,
    -- Finish all VeryLazy handlers, including LazyVim's autocmd setup, before restoring files.
    callback = vim.schedule_wrap(function()
      if vim.fn.argc() ~= 0 then return end

      local ok, persistence_module = pcall(require, "persistence")
      if ok and hasCurrentSession(persistence_module) then
        persistence_module.load()
        openStartupExplorer()
        return
      end

      Snacks.explorer({ cwd = vim.fn.getcwd() })
    end),
  })
end

return M
