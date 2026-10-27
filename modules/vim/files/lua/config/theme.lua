local exportedThemeName = vim.g.nix_flakes_theme or "one-half-light"
local fallbackExportedThemeName = "one-half-light"

local function load_exported_theme(theme_name)
  local theme_path = vim.fn.stdpath("config") .. "/themes/" .. theme_name .. ".lua"
  if not (vim.uv or vim.loop).fs_stat(theme_path) then
    return false, ("theme file not found: %s"):format(theme_path)
  end

  local ok, theme_or_err = pcall(dofile, theme_path)
  if not ok then
    return false, theme_or_err
  end
  if type(theme_or_err) ~= "table" or type(theme_or_err.setup) ~= "function" then
    return false, ("theme file does not export setup(): %s"):format(theme_path)
  end

  local setup_ok, setup_err = pcall(theme_or_err.setup)
  if not setup_ok then
    return false, setup_err
  end

  return true
end

return function()
  local ok, err = load_exported_theme(exportedThemeName)
  if ok then
    return
  end

  if exportedThemeName ~= fallbackExportedThemeName then
    vim.notify(
      ("Failed to load theme '%s': %s. Falling back to '%s'."):format(
        exportedThemeName,
        err,
        fallbackExportedThemeName
      ),
      vim.log.levels.WARN
    )

    local fallback_ok, fallback_err = load_exported_theme(fallbackExportedThemeName)
    if fallback_ok then
      return
    end

    err = fallback_err
  end

  vim.notify(
    ("Failed to load fallback theme '%s': %s"):format(fallbackExportedThemeName, err),
    vim.log.levels.ERROR
  )
end
