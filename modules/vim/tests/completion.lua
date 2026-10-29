local lazyvim = assert(vim.env.LAZYVIM_SOURCE, "Set LAZYVIM_SOURCE to a LazyVim checkout")
local plugins = vim.env.NVIM_PLUGIN_ROOT or (vim.fn.stdpath("data") .. "/lazy")
for _, path in ipairs({ lazyvim, plugins .. "/lazy.nvim", plugins .. "/blink.cmp", plugins .. "/minuet-ai.nvim" }) do
  vim.opt.rtp:append(path)
end

-- Fail before any subprocess or credential access, including accidental AI requests.
vim.system = function()
  error("Subprocesses are forbidden in the offline completion test")
end
package.loaded["config.ai"] = {
  getOpenAIKey = function()
    error("Credentials are forbidden in the offline completion test")
  end,
}
local requests, respond = 0, nil
package.loaded["minuet.backends.openai"] = {
  is_available = function()
    return true
  end,
  complete = function(_, callback)
    requests = requests + 1
    respond = callback
  end,
}

LazyVim = {
  cmp = require("lazyvim.util.cmp"),
  config = { icons = { kinds = {} } },
}
local Plugin = require("lazy.core.plugin")
local Config = require("lazy.core.config")
Config.options = vim.deepcopy(Config.defaults)
local upstream = dofile(lazyvim .. "/lua/lazyvim/plugins/extras/coding/blink.lua")
local personal = dofile("modules/vim/files/lua/plugins/completion.lua")
local spec = Plugin.Spec.new({ upstream, personal }, { pkg = false })
assert(#spec.notifs == 0, vim.inspect(spec.notifs))
local blink = assert(spec.plugins["blink.cmp"])
assert(vim.deep_equal(Plugin.values(spec.plugins["minuet-ai.nvim"], "event"), { "BufReadPre", "BufNewFile" }))
local minuet = require("minuet")
minuet.setup(Plugin.values(spec.plugins["minuet-ai.nvim"], "opts"))
local opts = Plugin.values(blink, "opts")
assert(blink.version == "1.*")
assert(vim.deep_equal(opts.sources.default, { "lsp", "path", "snippets", "buffer" }))
assert(opts.sources.per_filetype.lua.inherit_defaults and opts.sources.providers.lazydev)
assert(opts.completion.accept.auto_brackets.enabled and opts.cmdline.enabled)
assert(not opts.completion.ghost_text.enabled and not opts.completion.trigger.prefetch_on_insert)
assert(opts.sources.providers.minuet == nil)
assert(vim.tbl_contains(minuet.config.virtualtext.auto_trigger_ft, "nix"))
assert(minuet.config.virtualtext.show_on_completion_menu and not minuet.config.blink.enable_auto_complete)

-- Use Blink's Lua matcher so setup validates the real merged options without downloads.
opts.fuzzy = { implementation = "lua" }
blink.config(blink, opts)
assert(require("blink.cmp.config").keymap.preset == "super-tab")
assert(opts.keymap["<Tab>"][4] == "fallback")

local called = {}
local fakeCmp = {
  hide = function()
    called[#called + 1] = "hide"
  end,
  snippet_active = function()
    return false
  end,
  select_and_accept = function()
    called[#called + 1] = "menu"
    return true
  end,
  accept = function()
    called[#called + 1] = "selected"
    return true
  end,
}
assert(not opts.keymap["<Tab>"][1](fakeCmp) and #called == 0)
assert(opts.keymap["<Tab>"][2](fakeCmp) and called[1] == "menu")
fakeCmp.snippet_active = function()
  return true
end
assert(opts.keymap["<Tab>"][2](fakeCmp) and called[2] == "selected")
fakeCmp.accept = function()
  return nil
end
assert(not opts.keymap["<Tab>"][2](fakeCmp))
local jumped = false
vim.snippet.active = function()
  return true
end
vim.snippet.jump = function(direction)
  jumped = direction == 1
end
assert(opts.keymap["<Tab>"][3] == "snippet_forward")
assert(require("blink.cmp").snippet_forward())
vim.wait(10, function()
  return jumped
end)
assert(jumped, "An active snippet must advance when no menu item can be accepted")
local virtualtext = require("minuet.virtualtext")
-- Headless Lua runs outside insert mode; keep the actual autocmd and renderer paths.
vim.fn.mode = function()
  return "i"
end
vim.api.nvim_buf_set_lines(0, 0, -1, false, { "{ value = ; }" })
vim.api.nvim_win_set_cursor(0, { 1, 10 })
vim.bo.filetype = "nix"
assert(vim.b.minuet_virtual_text_auto_trigger)
vim.api.nvim_exec_autocmds("InsertEnter", { buffer = 0 })
assert(requests == 0, "Automatic requests must wait for debounce")
assert(vim.wait(500, function()
  return requests == 1
end), "Nix FileType and InsertEnter must automatically request inline AI")
respond({ "42" })
assert(virtualtext.action.is_visible(), "The response must render an inline suggestion")
local marks = vim.api.nvim_buf_get_extmarks(0, virtualtext.ns_id, 0, -1, { details = true })
assert(#marks == 1 and marks[1][4].virt_text_pos == "inline")
assert(vim.deep_equal(marks[1][4].virt_text, { { "42", "MinuetVirtualText" } }))
assert(vim.api.nvim_get_hl(0, { name = "MinuetVirtualText", link = true }).link == "Comment")
assert(vim.api.nvim_get_current_line() == "{ value = ; }", "Ghost text must not modify the buffer")
assert(opts.keymap["<Tab>"][1](fakeCmp) and called[3] == "hide")
assert(vim.wait(100, function()
  return vim.api.nvim_get_current_line() == "{ value = 42; }"
end), "Tab must insert the visible AI suggestion")
assert(not virtualtext.action.is_visible())
vim.api.nvim_exec_autocmds("CursorMovedI", { buffer = 0 })
vim.wait(minuet.config.debounce + 50)
assert(requests == 1, "Automatic requests must respect the existing throttle")

vim.api.nvim_exec_autocmds("InsertLeave", { buffer = 0 })
vim.api.nvim_set_current_buf(vim.api.nvim_create_buf(true, false))
vim.bo.filetype = "markdown"
assert(not vim.b.minuet_virtual_text_auto_trigger)
assert(opts.keymap["<C-y>"][1](fakeCmp))
assert(requests == 2, "Manual AI must bypass the automatic filetype filter, throttle, and debounce")
respond({ "offline AI suggestion" })
assert(virtualtext.action.is_visible())
assert(vim.api.nvim_get_current_line() == "")
virtualtext.action.dismiss()
vim.wait(minuet.config.throttle + 50)
vim.api.nvim_exec_autocmds("InsertEnter", { buffer = 0 })
vim.api.nvim_exec_autocmds("CursorMovedI", { buffer = 0 })
vim.wait(minuet.config.debounce + 50)
assert(requests == 2, "Markdown must not request automatic inline AI")
print("PASS: merged LazyVim/Blink setup, automatic Nix inline rendering, Comment highlight, Tab acceptance, menu/snippet fallback, Markdown filtering, throttle, and manual AI")
vim.cmd("qa!")
