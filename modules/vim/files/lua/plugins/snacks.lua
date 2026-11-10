local function sync_search_files()
  local buf = vim.api.nvim_get_current_buf()
  if vim.bo[buf].buftype == ""
      and vim.bo[buf].modified
      and vim.api.nvim_buf_get_name(buf) ~= ""
  then
    vim.cmd("update")
  end

  vim.cmd("checktime")
end

local function refresh_search_on_change(picker)
  local pending = false
  picker.list.win:on({ "BufWritePost", "FileChangedShellPost", "FocusGained" }, function()
    if pending then return end
    pending = true
    vim.schedule(function()
      if picker.closed then return end
      vim.cmd("checktime")
      pending = false
      picker:refresh()
      picker.preview:refresh(picker)
    end)
  end)
end

return {
  "folke/snacks.nvim",
  priority = 1000,
  lazy = false,
  ---@type snacks.Config
  opts = {
    scroll = { enabled = false },
    dashboard = { enabled = false },
    explorer = { enabled = true },
    picker = {
      enabled = true,
      sources = {
        grep = { on_show = refresh_search_on_change },
        grep_word = { on_show = refresh_search_on_change },
        grep_buffers = { on_show = refresh_search_on_change },
        explorer = {
          layout = { preset = "sidebar" },
          actions = {
            -- When the explorer is the sole window, open files in a new
            -- split instead of replacing the explorer.
            confirm_with_window = function(picker, item)
              if not item or item.dir then
                picker:action("confirm")
                return
              end
              -- Check for a non-sidebar edit window
              for _, w in ipairs(vim.api.nvim_list_wins()) do
                if vim.api.nvim_win_is_valid(w)
                    and vim.api.nvim_win_get_config(w).relative == ""
                    and not vim.w[w].snacks_layout
                then
                  picker:action("confirm")
                  return
                end
              end
              -- No edit window: open the file directly in a new split
              local file = item.file or item.text
              if not file then return end
              local explorer_win = vim.api.nvim_get_current_win()
              vim.cmd("botright vsplit " .. vim.fn.fnameescape(file))
              vim.schedule(function()
                if vim.api.nvim_win_is_valid(explorer_win) then
                  vim.api.nvim_win_set_width(explorer_win, 40)
                end
              end)
            end,
          },
          win = {
            list = {
              keys = {
                ["<CR>"] = "confirm_with_window",
                ["l"] = "confirm_with_window",
              },
            },
          },
        },
      },
    },
    notifier = { enabled = true },
    notify = { enabled = true },
    bigfile = { enabled = true },
    gh = { enabled = true },
    gitbrowse = { enabled = true },
    image = { enabled = true },
    indent = { enabled = true },
    input = { enabled = true },
    lazygit = { enabled = true },
    quickfile = { enabled = true },
    rename = { enabled = true },
    scope = { enabled = true },
    statuscolumn = { enabled = true },
    toggle = { enabled = true },
    words = { enabled = true },
  },
  keys = {
    -- Picker: search
    { "<leader>/",  function() sync_search_files(); LazyVim.pick.open("grep") end, desc = "Grep (Root Dir)" },
    { "<leader>sg", function() sync_search_files(); LazyVim.pick.open("live_grep") end, desc = "Grep (Root Dir)" },
    { "<leader>sw", function() sync_search_files(); LazyVim.pick.open("grep_word") end, desc = "Visual selection or word (Root Dir)", mode = { "n", "x" } },
    -- Picker: git
    { "<leader>gb", function() Snacks.picker.git_branches() end,              desc = "Git Branches" },
    { "<leader>gd", function() Snacks.picker.git_diff() end,                  desc = "Git Diff (Hunks)" },
    { "<leader>gf", function() Snacks.picker.git_log_file() end,              desc = "Git Log File" },
    { "<leader>gs", function() Snacks.picker.git_status() end,                desc = "Git Status" },
    { "<leader>gS", function() Snacks.picker.git_stash() end,                 desc = "Git Stash" },
    { "<leader>gL", function() Snacks.picker.git_log_line() end,              desc = "Git Log Line" },
    { "<leader>gB", function() Snacks.gitbrowse() end,                        desc = "Git Browse",                 mode = { "n", "v" } },
    -- Picker: GitHub
    { "<leader>gi", function() Snacks.picker.gh_issue() end,                  desc = "GitHub Issues (Open)" },
    { "<leader>gI", function() Snacks.picker.gh_issue({ state = "all" }) end, desc = "GitHub Issues (All)" },
    { "<leader>gp", function() Snacks.picker.gh_pr() end,                     desc = "GitHub Pull Requests (Open)" },
    { "<leader>gP", function() Snacks.picker.gh_pr({ state = "all" }) end,    desc = "GitHub Pull Requests (All)" },
    -- Picker: LSP
    { "gd",         function() Snacks.picker.lsp_definitions() end,           desc = "Goto Definition" },
    { "gb",         "<C-o>",                                                 desc = "Jump Back" },
    { "gr",         function() Snacks.picker.lsp_references() end,            nowait = true,                       desc = "References" },
    -- Picker: misc
    { "<leader>:",  function() Snacks.picker.command_history() end,           desc = "Command History" },
    { "<leader>cR", function() Snacks.rename.rename_file() end,               desc = "Rename File" },
    { "<leader>fh", function() Snacks.picker.help() end,                      desc = "Help Pages" },
    -- Notifier
    { "<leader>n",  function() Snacks.notifier.show_history() end,            desc = "Notification History" },
  },
}
