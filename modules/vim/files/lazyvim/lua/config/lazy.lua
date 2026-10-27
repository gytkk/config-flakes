local lazy = require("config.legacy")("lazy")
vim.opt.rtp:prepend(lazy)

require("lazy").setup({
  spec = {
    { "LazyVim/LazyVim", version = "16.0.1", import = "lazyvim.plugins" },
    { import = "plugins" },
  },
  lockfile = vim.g.nvim_candidate_root .. "/lazy-lock.json",
  performance = { rtp = { paths = { vim.g.nvim_candidate_root } } },
  checker = { enabled = false },
  change_detection = { notify = false },
})
