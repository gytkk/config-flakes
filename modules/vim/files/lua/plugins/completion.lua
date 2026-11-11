local automaticFiletypes = {
  "bash",
  "go",
  "gomod",
  "gosum",
  "hcl",
  "javascript",
  "javascriptreact",
  "json",
  "lua",
  "nix",
  "python",
  "rust",
  "sh",
  "terraform",
  "toml",
  "tsx",
  "typescript",
  "typescriptreact",
  "yaml",
  "zsh",
}

return {
  {
    "milanglacier/minuet-ai.nvim",
    event = { "BufReadPre", "BufNewFile" },
    opts = {
      provider = "openai",
      n_completions = 1,
      context_window = 6000,
      throttle = 900,
      debounce = 180,
      request_timeout = 4,
      notify = "warn",
      after_cursor_filter_length = 24,
      before_cursor_filter_length = 4,
      enable_predicates = {
        function()
          return vim.tbl_contains(automaticFiletypes, vim.bo.filetype)
        end,
      },
      blink = { enable_auto_complete = false },
      virtualtext = {
        auto_trigger_ft = automaticFiletypes,
        show_on_completion_menu = true,
      },
      provider_options = {
        openai = {
          model = "gpt-6-luna",
          api_key = require("config.ai").getOpenAIKey,
          optional = {
            -- The completion budget includes reasoning tokens.
            max_completion_tokens = 1024,
            reasoning_effort = "none",
            service_tier = "fast",
          },
        },
      },
    },
  },
  {
    "saghen/blink.cmp",
    version = "1.*",
    dependencies = { "milanglacier/minuet-ai.nvim" },
    opts = function(_, opts)
      opts.keymap.preset = "super-tab"
      opts.keymap["<Tab>"] = require("blink.cmp.keymap.presets").get("super-tab")["<Tab>"]
      table.insert(opts.keymap["<Tab>"], 1, function(cmp)
        local suggestion = require("minuet.virtualtext").action
        if suggestion.is_visible() then
          cmp.hide()
          suggestion.accept()
          return true
        end
      end)
      opts.keymap["<C-y>"] = {
        function()
          require("minuet.virtualtext").action.next()
          return true
        end,
      }
      opts.completion.documentation.auto_show_delay_ms = 300
      opts.completion.ghost_text.enabled = false
      opts.completion.trigger = vim.tbl_extend("force", opts.completion.trigger or {}, {
        prefetch_on_insert = false,
      })
      opts.signature = vim.tbl_extend("force", opts.signature or {}, { enabled = true })
    end,
  },
}
