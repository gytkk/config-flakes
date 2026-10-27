Snacks.toggle.diagnostics():map("<leader>ud")
Snacks.toggle.inlay_hints():map("<leader>uh")
Snacks.toggle.line_number():map("<leader>ul")
Snacks.toggle.scroll():map("<leader>uS")
Snacks.toggle.option("wrap", { name = "Wrap" }):map("<leader>uw")

local diagnosticWin
vim.keymap.set("n", "<leader>d", function()
  if diagnosticWin and vim.api.nvim_win_is_valid(diagnosticWin) then
    vim.api.nvim_win_close(diagnosticWin, true)
    diagnosticWin = nil
  else
    local _, win = vim.diagnostic.open_float({ scope = "line" })
    diagnosticWin = win
  end
end, { desc = "Toggle Line Diagnostics" })
vim.keymap.set("n", "<leader>q", vim.diagnostic.setloclist, { desc = "Diagnostics List" })
