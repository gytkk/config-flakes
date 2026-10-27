local root = vim.fs.dirname(vim.g.nvim_candidate_root) .. "/config/"

-- Reuse personal behavior while the candidate is compared with the active configuration.
return function(name)
  return dofile(root .. name:gsub("%.", "/") .. ".lua")
end
