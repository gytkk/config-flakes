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
local candidate = dofile("modules/vim/files/lazyvim/lua/plugins/completion.lua")
local spec = Plugin.Spec.new({ upstream, candidate }, { pkg = false })
assert(#spec.notifs == 0, vim.inspect(spec.notifs))
local blink = assert(spec.plugins["blink.cmp"])
local minuet = require("minuet")
minuet.setup(Plugin.values(spec.plugins["minuet-ai.nvim"], "opts"))
local opts = Plugin.values(blink, "opts")
assert(blink.version == "1.*")
assert(vim.deep_equal(opts.sources.default, { "lsp", "path", "snippets", "buffer", "minuet" }))
assert(opts.sources.per_filetype.lua.inherit_defaults and opts.sources.providers.lazydev)
assert(opts.completion.accept.auto_brackets.enabled and opts.cmdline.enabled)
assert(not opts.completion.ghost_text.enabled and not opts.completion.trigger.prefetch_on_insert)
assert(opts.sources.providers.minuet.async and opts.sources.providers.minuet.timeout_ms == 4000)
assert(opts.sources.providers.minuet.score_offset < 0)
assert(vim.deep_equal(opts.completion.menu.draw.columns[3], { "source_name" }))
assert(#minuet.config.virtualtext.auto_trigger_ft == 0 and minuet.config.blink.enable_auto_complete)

-- Use Blink's Lua matcher so setup validates the real merged options without downloads.
opts.fuzzy = { implementation = "lua" }
blink.config(blink, opts)
assert(require("blink.cmp.config").keymap.preset == "super-tab")
assert(opts.keymap["<Tab>"][3] == "fallback")

local called = {}
local fakeCmp = {
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
assert(opts.keymap["<Tab>"][1](fakeCmp) and called[1] == "menu")
fakeCmp.snippet_active = function()
  return true
end
assert(opts.keymap["<Tab>"][1](fakeCmp) and called[2] == "selected")
fakeCmp.accept = function()
  return nil
end
assert(not opts.keymap["<Tab>"][1](fakeCmp))
local jumped = false
vim.snippet.active = function()
  return true
end
vim.snippet.jump = function(direction)
  jumped = direction == 1
end
assert(opts.keymap["<Tab>"][2]())
vim.wait(10, function()
  return jumped
end)
assert(jumped, "An active snippet must advance when no menu item can be accepted")
local shown
opts.keymap["<C-y>"][1]({ show = function(options)
  shown = options.providers
end })
assert(vim.deep_equal(shown, { "minuet" }))

local utils = require("minuet.utils")
utils.make_cmp_context = function()
  return {}
end
utils.get_context = function()
  return { lines_before = "", lines_after = "" }
end
local source = require("minuet.blink").new()
local completed
vim.bo.filetype = "markdown"
source:get_completions({ trigger = { kind = "keyword" } }, function()
  completed = true
end)
vim.wait(220)
assert(completed and requests == 0, "Automatic AI must respect the filetype allowlist")
vim.bo.filetype = "lua"
source:get_completions({ trigger = { kind = "keyword" } }, function() end)
assert(vim.wait(300, function()
  return requests == 1
end), "An allowlisted filetype must request automatic AI after debounce")
source:get_completions({ trigger = { kind = "keyword" } }, function() end)
vim.wait(220)
assert(requests == 1, "Automatic requests must respect the existing throttle")
vim.bo.filetype = "markdown"
local result
source:get_completions({ trigger = { kind = "manual" } }, function(value)
  result = value
end)
assert(requests == 2, "Manual AI must bypass the automatic filetype filter, throttle, and debounce")
respond({ "offline AI suggestion" })
assert(result.items[1].insertText == "offline AI suggestion")
assert(not require("minuet.virtualtext").action.is_visible())
assert(requests == 2, "Only Blink's source should have requested AI")
print("PASS: merged LazyVim/Blink setup, Tab/snippet priority, manual AI, filetype filtering, throttle, and no inline display")
vim.cmd("qa!")
