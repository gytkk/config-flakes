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
    event = "InsertEnter",
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
      blink = { enable_auto_complete = true },
      -- Blink owns automatic requests and presentation, including AI suggestions.
      virtualtext = { auto_trigger_ft = {} },
      provider_options = {
        openai = {
          model = "gpt-6.1-terra",
          api_key = require("config.ai").getOpenAIKey,
          optional = {
            -- The completion budget includes reasoning tokens.
            max_completion_tokens = 1024,
            reasoning_effort = "low",
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
      opts.keymap["<C-y>"] = require("minuet").make_blink_map()
      opts.completion.documentation.auto_show_delay_ms = 300
      opts.completion.ghost_text.enabled = false
      local draw = opts.completion.menu.draw
      draw.columns = draw.columns or { { "kind_icon" }, { "label", "label_description", gap = 1 } }
      table.insert(draw.columns, { "source_name" })
      opts.completion.trigger = vim.tbl_extend("force", opts.completion.trigger or {}, {
        prefetch_on_insert = false,
      })
      table.insert(opts.sources.default, "minuet")
      opts.sources.providers = opts.sources.providers or {}
      opts.sources.providers.minuet = {
        name = "minuet",
        module = "minuet.blink",
        async = true,
        timeout_ms = 4000,
        score_offset = -50,
      }
      opts.signature = vim.tbl_extend("force", opts.signature or {}, { enabled = true })
    end,
  },
}
