local root = vim.fn.getcwd()
local pluginRoot = vim.env.NVIM_PLUGIN_ROOT or (vim.fn.stdpath("data") .. "/lazy")
local temp = vim.fn.tempname()
vim.fn.mkdir(temp, "p")

local function run(command, timeout)
  local result
  -- HTTP requests need Neovim to keep servicing backend RPC calls.
  local process = vim.system(command, { text = true }, function(output)
    result = output
  end)
  if not vim.wait(timeout, function() return result ~= nil end) then
    process:kill(9)
    vim.wait(2000, function() return result ~= nil end)
    error("Timed out running " .. command[1] .. ": " .. (result and result.stderr or "no process output"))
  end
  return result
end

local function check()
  vim.go.loadplugins = true
  vim.opt.rtp:prepend(pluginRoot .. "/lazy.nvim")
  local spec
  for _, candidate in ipairs(dofile(root .. "/modules/vim/files/lua/plugins/editing.lua")) do
    if candidate[1] == "iamcco/markdown-preview.nvim" then
      spec = candidate
    end
  end
  assert(spec, "Markdown preview spec missing")
  require("lazy").setup({
    root = pluginRoot,
    spec = { spec },
    lockfile = root .. "/modules/vim/files/lazy-lock.json",
    state = temp .. "/lazy-state.json",
    install = { missing = false },
    checker = { enabled = false },
    change_detection = { enabled = false },
  })
  assert(vim.fn.exists(":MarkdownViewer") == 2)
  assert(not require("lazy.core.config").plugins["markdown-preview.nvim"]._.loaded)

  -- Capture the URL instead of opening the user's browser during the check.
  vim.cmd([[
    function! MarkdownPreviewTestBrowser(url) abort
      let g:markdown_preview_test_url = a:url
    endfunction
  ]])
  vim.g.mkdp_browserfunc = "MarkdownPreviewTestBrowser"
  local fixture = temp .. "/preview.md"
  vim.fn.writefile({ "# Browser preview check", "", "A **rendered** Markdown paragraph." }, fixture)
  vim.cmd("filetype on")
  vim.cmd.edit(fixture)
  assert(vim.bo.filetype == "markdown")
  assert(vim.g.mkdp_auto_start == 0)
  assert(vim.fn["mkdp#rpc#get_server_status"]() == -1, "Opening Markdown must not start the backend")

  vim.cmd("MarkdownViewer")
  assert(vim.wait(10000, function()
    return vim.g.markdown_preview_test_url ~= nil
  end), "Preview did not provide a browser URL")
  local url = vim.g.markdown_preview_test_url
  assert(url:match("^http://localhost:%d+/page/%d+$"), "Expected a localhost preview URL: " .. url)
  assert(vim.fn["mkdp#rpc#get_server_status"]() == 1)
  local response = run({ "curl", "--fail", "--silent", "--show-error", url }, 10000)
  assert(response.code == 0, "Preview HTTP request failed: " .. (response.stderr or ""))
  assert(response.stdout:find("<html", 1, true), "Preview must serve an HTML document")

  vim.cmd("MarkdownPreviewStop")
  assert(vim.fn["mkdp#rpc#get_server_status"]() == -1, "Stop must shut down the backend")
  print("PASS: lazy command, no automatic backend, MarkdownViewer alias, localhost HTTP preview, stop")
end

local ok, err = xpcall(check, debug.traceback)
if vim.fn.exists("*mkdp#rpc#stop_server") == 1 then
  vim.fn["mkdp#rpc#stop_server"]()
end
vim.fn.delete(temp, "rf")
if not ok then
  io.stderr:write(err .. "\n")
  vim.cmd("cquit 1")
end
vim.cmd("qa!")
