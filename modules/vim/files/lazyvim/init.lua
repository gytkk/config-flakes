local root = vim.fs.dirname(debug.getinfo(1, "S").source:sub(2))
vim.g.nvim_candidate_root = root
vim.opt.rtp:prepend(root)

assert(vim.env.NVIM_APPNAME == "nvim-lazyvim", "Use modules/vim/try-lazyvim.sh to isolate the candidate")
local legacyConfig = assert(vim.env.NVIM_LEGACY_CONFIG, "Missing NVIM_LEGACY_CONFIG")
local init = table.concat(vim.fn.readfile(legacyConfig .. "/init.lua"), "\n")
-- Read only the JSON literals emitted by Home Manager, without executing the old init.
for _, name in ipairs({ "nix_flakes_theme", "openai_api_key_path" }) do
  local value = assert(init:match("vim%.g%." .. name .. "%s*=%s*([^\n]+)"), "Missing Home Manager setting: " .. name)
  vim.g[name] = vim.json.decode(value)
end
for _, provider in ipairs({ "node", "perl", "ruby", "python3" }) do
  vim.g["loaded_" .. provider .. "_provider"] = 0
end

require("config.lazy")
