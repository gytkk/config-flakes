local M = {}

local openaiApiKey

local function resolveAgenixPath(path)
  local resolved = path

  if resolved:find("${XDG_RUNTIME_DIR}", 1, true) then
    local runtimeDir = (vim.env.XDG_RUNTIME_DIR or ""):gsub("/+$", "")
    resolved = resolved:gsub("%${XDG_RUNTIME_DIR}", runtimeDir)
  end

  local command = resolved:match("%$%((.-)%)")
  if command then
    local result = vim.system({ "sh", "-lc", command }, { text = true }):wait()
    if result.code ~= 0 then
      return nil, ("Failed to resolve agenix runtime directory with `%s`."):format(command)
    end

    local runtimeDir = vim.trim(result.stdout or ""):gsub("/+$", "")
    resolved = resolved:gsub("%$%((.-)%)", runtimeDir, 1)
  end

  return resolved
end

local function readOpenAIKeyFile(path)
  if type(path) ~= "string" or path == "" then
    return nil, "Agenix secret path is not configured."
  end

  local resolvedPath, pathErr = resolveAgenixPath(path)
  if not resolvedPath then
    return nil, pathErr
  end

  if not vim.uv.fs_stat(resolvedPath) then
    return nil, ("Agenix secret file does not exist at %s."):format(resolvedPath)
  end

  local ok, lines = pcall(vim.fn.readfile, resolvedPath)
  if not ok then
    return nil, ("Failed to read agenix secret at %s."):format(resolvedPath)
  end

  local value = vim.trim(table.concat(lines, "\n"))
  if value == "" then
    return nil, ("Agenix secret at %s is empty."):format(resolvedPath)
  end

  return value
end

function M.getOpenAIKey()
  if openaiApiKey then
    return openaiApiKey
  end

  local value, err = readOpenAIKeyFile(vim.g.openai_api_key_path)
  if value then
    openaiApiKey = value
    return openaiApiKey
  end

  error(
    "Failed to read OPENAI_API_KEY from agenix. "
      .. "Create secrets/openai-api-key.age, run home-manager switch, and ensure Neovim receives "
      .. "vim.g.openai_api_key_path. Last error: "
      .. err
  )
end

return M
