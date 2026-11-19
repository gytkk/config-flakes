-- Run headless with -u NONE -i NONE --noplugin from the repository root.
local tmp = vim.fn.tempname()
local mtime = os.time()

local function write(path, text)
  assert(vim.fn.writefile({ text }, path) == 0)
  mtime = mtime + 1 -- Distinguish successive writes even on coarse timestamp filesystems.
  assert(vim.uv.fs_utime(path, mtime, mtime))
end

local function line(buf)
  return vim.api.nvim_buf_get_lines(buf, 0, 1, false)[1]
end

local function check_line(buf, expected)
  local actual = line(buf)
  assert(actual == expected, ("buffer %d: expected %q, got %q"):format(buf, expected, actual))
end

-- Exercise real Neovim hints with an in-process transport, not a language server.
local function start_hint_client(buf, read_from_disk)
  local requests, server_text = 0, nil
  local client = assert(vim.lsp.start({
    name = read_from_disk and "reload-disk-fixture" or "reload-sync-fixture",
    cmd = function(dispatchers)
      return {
        request = function(method, _, reply, done)
          if method == "initialize" then
            reply(nil, { capabilities = {
              inlayHintProvider = true,
              textDocumentSync = { openClose = not read_from_disk, change = read_from_disk and 0 or 1 },
            } })
          elseif method == "textDocument/inlayHint" then
            requests = requests + 1
            local id = requests
            local text = read_from_disk and vim.fn.readfile(vim.api.nvim_buf_get_name(buf))[1] or server_text
            local hint = { position = { line = 0, character = 1 }, label = text }
            vim.schedule(function()
              done(id)
              reply(nil, { hint }, id)
            end)
            return true, id
          elseif method == "shutdown" then
            reply(nil, nil)
          end
          return true, 0
        end,
        notify = function(method, params)
          if method == "textDocument/didOpen" then
            server_text = vim.trim(params.textDocument.text)
          elseif method == "exit" then
            dispatchers.on_exit(0, 0)
          end
          return true
        end,
        is_closing = function() return false end,
        terminate = function() dispatchers.on_exit(0, 0) end,
      }
    end,
  }, { bufnr = buf }))
  vim.lsp.inlay_hint.enable(true, { bufnr = buf })
  local function has_hint(text)
    local hints = vim.lsp.inlay_hint.get({ bufnr = buf })
    return #hints == 1 and hints[1].inlay_hint.label == text
  end
  assert(vim.wait(1000, function() return has_hint(line(buf)) end), "initial hint missing")
  return vim.lsp.get_client_by_id(client), has_hint, function() return requests end
end

local function run()
  vim.opt.autoread = true
  vim.opt.swapfile = false
  vim.opt.undofile = false
  vim.opt.backup = false
  vim.opt.writebackup = false
  vim.cmd("filetype on")
  vim.cmd("syntax on")
  vim.api.nvim_create_augroup("lazyvim_checktime", {})
  dofile("modules/vim/files/lua/config/autocmds.lua")

  local a_path, b_path = tmp .. "/a.lua", tmp .. "/b.lua"
  write(a_path, "-- old needle")
  write(b_path, "-- old comment")
  vim.cmd.edit(b_path)
  local b = vim.api.nvim_get_current_buf()
  local b_win = vim.api.nvim_get_current_win()
  assert(vim.fn.synIDattr(vim.fn.synID(1, 1, 1), "name") == "luaComment")
  local disk_client, has_disk_hint = start_hint_client(b, true)
  vim.cmd.split(a_path)
  local a = vim.api.nvim_get_current_buf()
  vim.treesitter.start(a)
  local highlighter = assert(vim.treesitter.highlighter.active[a])
  assert(vim.treesitter.get_captures_at_pos(a, 0, 0)[1].capture == "comment")

  local sync_client, has_hint, requests = start_hint_client(a, false)
  local before = requests()

  vim.fn.setreg("/", "needle")
  vim.v.hlsearch = 1
  assert(vim.v.hlsearch == 1)
  write(a_path, "local needle = 1")
  write(b_path, "local first = 1")
  vim.api.nvim_exec_autocmds("CursorHold", { modeline = false })
  -- Check scope before yielding to native autoread file watchers.
  check_line(a, "local needle = 1")
  check_line(b, "-- old comment")
  assert(vim.treesitter.highlighter.active[a] == highlighter, "Tree-sitter highlighter was rebuilt")
  assert(vim.treesitter.get_captures_at_pos(a, 0, 0)[1].capture == "keyword")
  assert(vim.wait(1000, function()
    return has_hint("local needle = 1") and vim.v.hlsearch == 0
  end), "reload did not update hints and clear search highlights")
  assert(requests() == before + 1, "expected one hint request for the reload")
  assert(vim.fn.getreg("/") == "needle", "reload discarded the search pattern")

  write(a_path, "local needle = 2")
  write(b_path, "local second = 2")
  local old_b = line(b)
  vim.api.nvim_exec_autocmds("CursorHoldI", { modeline = false })
  check_line(a, "local needle = 2")
  check_line(b, old_b)

  write(a_path, "local needle = 3")
  vim.api.nvim_set_current_win(b_win)
  check_line(a, "local needle = 2")
  check_line(b, "local second = 2")
  assert(vim.bo[b].syntax == "lua" and not vim.treesitter.highlighter.active[b])
  assert(vim.fn.synIDattr(vim.fn.synID(1, 1, 1), "name") == "luaStatement")

  -- The focus check runs on the next loop turn; replace autoread watchers with
  -- an explicit reload choice so they cannot mask a missing global check.
  vim.opt.autoread = false
  vim.api.nvim_create_autocmd("FileChangedShell", {
    callback = function() vim.v.fcs_choice = "reload" end,
  })
  write(a_path, "local needle = 4")
  write(b_path, "local final = 4")
  vim.api.nvim_exec_autocmds("FocusGained", { modeline = false })
  assert(vim.wait(1000, function()
    return line(a) == "local needle = 4" and line(b) == "local final = 4"
  end), "focus did not reload both visible buffers")
  assert(vim.wait(1000, function() return has_disk_hint("local final = 4") end), "hint fallback did not refresh")
  sync_client:stop()
  disk_client:stop()
end

assert(vim.fn.mkdir(tmp) == 1)
local ok, err = xpcall(run, debug.traceback)
vim.fn.delete(tmp, "rf")
assert(ok, err)
io.stdout:write("PASS: scoped idle and entry checks, global focus reload, search, highlighting, and inlay hints\n")
