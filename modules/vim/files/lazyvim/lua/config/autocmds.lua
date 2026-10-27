vim.api.nvim_del_augroup_by_name("lazyvim_checktime")
vim.api.nvim_del_augroup_by_name("lazyvim_wrap_spell")
require("config.legacy")("autocmds").setup()

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
