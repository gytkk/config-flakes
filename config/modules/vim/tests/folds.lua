dofile("config/modules/vim/files/lua/config/options.lua")
dofile("config/modules/vim/files/lua/config/session.lua").setup()

vim.wo.foldlevel = 0
vim.api.nvim_buf_set_name(0, "fold-test-original")
vim.cmd.edit("config/modules/vim/files/lua/config/options.lua")
assert(vim.wo.foldlevel == 99, "New buffers must start with folds open")

vim.api.nvim_buf_set_lines(0, 0, -1, false, { "outer", "  inner", "    body", "  end", "end" })
vim.wo.foldmethod = "manual"
vim.cmd("1,5fold")
vim.cmd("2,4fold")
vim.cmd.tabnew()
vim.api.nvim_buf_set_lines(0, 0, -1, false, { "start", "body", "end" })
vim.wo.foldmethod = "manual"
vim.cmd("1,3fold")

for _, win in ipairs(vim.api.nvim_list_wins()) do
  vim.api.nvim_win_call(win, function()
    assert(vim.fn.foldclosed(1) == 1, "Fixture must contain closed folds")
  end)
end

vim.api.nvim_exec_autocmds("User", { pattern = "PersistenceLoadPost" })
for _, win in ipairs(vim.api.nvim_list_wins()) do
  vim.api.nvim_win_call(win, function()
    assert(vim.wo.foldlevel == 99)
    assert(vim.fn.foldclosed(1) == -1 and vim.fn.foldclosed(2) == -1,
      "Session restore must open nested folds in every tab")
    vim.cmd("normal! zM")
    assert(vim.fn.foldclosed(1) == 1, "Manual folding must remain available")
  end)
end

io.stdout:write("PASS: new buffers and restored sessions start unfolded; manual folding remains available\n")
