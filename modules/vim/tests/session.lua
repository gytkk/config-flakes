local root = vim.fn.getcwd()
local temp = vim.fn.tempname()
local workspace = temp .. "/workspace"
local state = temp .. "/state"
vim.fn.mkdir(workspace, "p")
workspace = assert(vim.uv.fs_realpath(workspace))
vim.fn.mkdir(state .. "/nvim/sessions", "p")
local child

local function check()
  local external = temp .. "/external"
  vim.fn.mkdir(external .. "/.git", "p")
  vim.fn.writefile({ "External settings" }, external .. "/settings.txt")
  assert(vim.uv.fs_symlink(external .. "/settings.txt", temp .. "/settings.txt"))
  vim.fn.writefile({ "# Markdown", string.rep("Long paragraph ", 30) }, workspace .. "/readme.md")
  vim.fn.writefile({ "# MDX", "Another document" }, workspace .. "/notes.mdx")
  vim.fn.writefile({ "return {}" }, workspace .. "/code.lua")
  vim.fn.writefile({ "description: " .. string.rep("Long paragraph ", 30) }, workspace .. "/config.yaml")
  vim.api.nvim_set_current_dir(workspace)
  vim.cmd.edit("readme.md")
  vim.cmd.vsplit()
  vim.cmd.tabedit("notes.mdx")
  vim.cmd.vsplit("code.lua")
  vim.cmd.tabedit("config.yaml")
  vim.cmd.tabedit(temp .. "/settings.txt")
  vim.opt.sessionoptions = { "buffers", "curdir", "tabpages", "winsize", "help", "globals", "skiprtp", "folds" }
  local session = state .. "/nvim/sessions/" .. workspace:gsub("[\\/:]+", "%%") .. ".vim"
  vim.cmd("mksession " .. vim.fn.fnameescape(session))
  assert(vim.fn.filereadable(session) == 1, "Session fixture must use Persistence's encoded filename")

  -- Attach a UI so startup follows the real bare vi path, including its UI guard.
  child = vim.fn.jobstart({
    vim.v.progpath, "--embed", "-i", "NONE",
    "--cmd", "autocmd User PersistenceLoadPost let g:session_test_loaded = 1",
    "--cmd", [[lua
      vim.api.nvim_create_autocmd("User", {
        pattern = "LazyVimAutocmds", once = true,
        callback = function()
          vim.api.nvim_create_autocmd("FileType", {
            callback = function(event) vim.b[event.buf].session_test_filetype = true end,
          })
        end,
      })
      vim.api.nvim_create_autocmd("User", {
        pattern = "PersistenceLoadPre",
        callback = function()
          vim.g.session_test_ready = package.loaded["config.autocmds"] ~= nil
            and package.loaded["config.keymaps"] ~= nil
        end,
      })
    ]],
  }, { rpc = true, cwd = workspace, env = { XDG_STATE_HOME = state } })
  assert(child > 0, "Could not start the session test Neovim")
  vim.rpcrequest(child, "nvim_ui_attach", 80, 24, { rgb = true })
  local ready = vim.wait(10000, function()
    return vim.rpcrequest(child, "nvim_exec_lua", [[
      return vim.g.session_test_loaded == 1 and package.loaded["config.autocmds"] ~= nil
    ]], {})
  end, 50)
  assert(ready, "Session restoration and deferred autocmd loading did not finish: " .. vim.inspect(
    vim.rpcrequest(child, "nvim_exec_lua", [[
      return { restored = vim.g.session_test_loaded, autocmds = package.loaded["config.autocmds"] ~= nil,
        state = vim.fn.stdpath("state"), session = require("persistence").current(),
        readable = vim.fn.filereadable(require("persistence").current()),
        messages = vim.api.nvim_exec2("messages", {output=true}).output }
    ]], {})))

  local verify = [[
    assert(vim.fn.argc() == 0, "Startup must have no file arguments")
    assert(vim.g.session_test_ready, "LazyVim setup must finish before session restoration starts")
    local markdown, yaml, code = 0, 0, 0
    for _, win in ipairs(vim.api.nvim_list_wins()) do
      local buf = vim.api.nvim_win_get_buf(win)
      if vim.bo[buf].buftype == "" then
        assert(vim.b[buf].session_test_filetype, "FileType hooks must run during restoration")
      end
      if vim.bo[buf].filetype == "markdown" then
        markdown = markdown + 1
        assert(vim.wo[win].wrap and vim.wo[win].linebreak,
          "Restored Markdown/MDX window must wrap: " .. vim.api.nvim_buf_get_name(buf))
        assert(not vim.wo[win].spell and vim.wo[win].foldlevel == 99)
        assert(vim.b[buf].autoformat == false)
      elseif vim.bo[buf].filetype == "yaml" then
        yaml = yaml + 1
        assert(vim.wo[win].wrap and vim.wo[win].linebreak, "Restored YAML window must wrap")
        assert(vim.b[buf].autoformat == false)
      elseif vim.bo[buf].filetype == "lua" then
        code = code + 1
        assert(not vim.wo[win].wrap, "Code must retain its wrapping default")
      end
    end
    assert(markdown == 3 and yaml == 1 and code == 1, "Restore every split and tab in the fixture")
    assert(vim.opt.listchars:get().trail == " ")
  ]]
  vim.rpcrequest(child, "nvim_exec_lua", verify, {})
  vim.rpcrequest(child, "nvim_exec_lua", [[
    local workspace = ...
    assert(vim.fn.getcwd() == workspace, "Restore the launching directory's session")
    assert(LazyVim.root() ~= workspace, "Fixture must restore a symlink into another project")
    local explorers = Snacks.picker.get({ source = "explorer", tab = false })
    assert(#explorers == 1 and explorers[1]:cwd() == workspace,
      "Startup explorer must stay in the launching directory even when the restored file belongs to another project")
  ]], { workspace })
  vim.rpcrequest(child, "nvim_exec_lua", [[
    for _, win in ipairs(vim.api.nvim_list_wins()) do
      vim.wo[win].wrap = false
      vim.wo[win].spell = true
    end
    require("persistence").load()
  ]], {})
  vim.rpcrequest(child, "nvim_exec_lua", verify, {})
end

local ok, err = xpcall(check, debug.traceback)
if child and child > 0 then
  vim.fn.jobstop(child)
  vim.fn.jobwait({ child }, 5000)
end
vim.api.nvim_set_current_dir(root)
vim.fn.delete(temp, "rf")
assert(ok, err)
io.stdout:write("PASS: startup explorer stays in the working directory; restored Markdown/MDX/YAML windows retain their settings\n")
