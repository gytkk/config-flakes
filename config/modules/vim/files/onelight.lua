-- onelight.lua — Neovim colorscheme matching Zed's "One Half Light Custom"
-- Source: modules/zed/themes/one-half-light.json

local M = {}

local p = {
  -- Syntax
  red = "#e45649",
  orange = "#d65d0e",
  yellow = "#c18401",
  green = "#50a14f",
  cyan = "#427b58",
  blue = "#0184bc",
  bright_yellow = "#e5c07b",

  -- UI
  fg = "#383a42",
  bg = "#fdfdfd",
  bg_float = "#f5f5f5",
  bg_alt = "#f0f0f0",
  border = "#e5e5e5",
  hover = "#e8e8e8",
  comment = "#a0a1a7",
  muted = "#4f525e",
  linenr = "#9d9d9f",
  selection = "#dee6ff",
  search = "#f1dfbc",
  search_active = "#98cde3",
  none = "NONE",
}

function M.setup()
  vim.cmd("hi clear")
  if vim.fn.exists("syntax_on") then
    vim.cmd("syntax reset")
  end
  vim.o.termguicolors = true
  vim.o.background = "light"
  vim.g.colors_name = "onelight"

  local hl = function(group, opts)
    vim.api.nvim_set_hl(0, group, opts)
  end

  -- Editor
  hl("Normal", { fg = p.fg, bg = p.bg })
  hl("NormalFloat", { fg = p.fg, bg = p.bg_float })
  hl("FloatBorder", { fg = p.border, bg = p.bg_float })
  hl("Cursor", { fg = p.blue, reverse = true })
  hl("CursorLine", { bg = p.bg_alt })
  hl("CursorColumn", { bg = p.bg_alt })
  hl("ColorColumn", { bg = p.bg_alt })
  hl("LineNr", { fg = p.linenr })
  hl("CursorLineNr", { fg = p.fg, bold = true })
  hl("SignColumn", { bg = p.bg })
  hl("VertSplit", { fg = p.border })
  hl("WinSeparator", { fg = p.border })
  hl("StatusLine", { fg = p.fg, bg = p.bg_float })
  hl("StatusLineNC", { fg = p.muted, bg = p.bg_alt })
  hl("TabLine", { fg = p.muted, bg = p.bg_alt })
  hl("TabLineFill", { bg = p.bg_alt })
  hl("TabLineSel", { fg = p.fg, bg = p.bg, bold = true })
  hl("Pmenu", { fg = p.fg, bg = p.bg_float })
  hl("PmenuSel", { bg = p.selection })
  hl("PmenuSbar", { bg = p.bg_alt })
  hl("PmenuThumb", { bg = p.border })
  hl("Visual", { bg = p.selection })
  hl("VisualNOS", { bg = p.selection })
  hl("Search", { bg = p.search })
  hl("IncSearch", { bg = p.search_active })
  hl("CurSearch", { bg = p.search_active, bold = true })
  hl("MatchParen", { fg = p.blue, underline = true })
  hl("Folded", { fg = p.comment, bg = p.bg_float })
  hl("FoldColumn", { fg = p.linenr, bg = p.bg })
  hl("NonText", { fg = p.border })
  hl("SpecialKey", { fg = p.border })
  hl("Whitespace", { fg = p.border })
  hl("EndOfBuffer", { fg = p.bg_float })
  hl("Directory", { fg = p.blue })
  hl("Title", { fg = p.blue, bold = true })
  hl("Question", { fg = p.green })
  hl("MoreMsg", { fg = p.green })
  hl("WarningMsg", { fg = p.yellow, bold = true })
  hl("ErrorMsg", { fg = p.red, bold = true })
  hl("ModeMsg", { fg = p.fg, bold = true })
  hl("WildMenu", { bg = p.selection })
  hl("Conceal", { fg = p.comment })
  hl("SpellBad", { undercurl = true, sp = p.red })
  hl("SpellCap", { undercurl = true, sp = p.yellow })
  hl("SpellRare", { undercurl = true, sp = p.orange })
  hl("SpellLocal", { undercurl = true, sp = p.cyan })

  -- Diff
  hl("DiffAdd", { bg = "#d4edda" })
  hl("DiffChange", { bg = "#fff3cd" })
  hl("DiffDelete", { bg = "#f8d7da" })
  hl("DiffText", { bg = "#ffeeba", bold = true })
  hl("Added", { fg = p.green })
  hl("Changed", { fg = p.yellow })
  hl("Removed", { fg = p.red })

  -- Syntax (Vim legacy groups)
  hl("Comment", { fg = p.comment, italic = true })
  hl("Constant", { fg = p.yellow })
  hl("String", { fg = p.green })
  hl("Character", { fg = p.green })
  hl("Number", { fg = p.yellow })
  hl("Boolean", { fg = p.yellow })
  hl("Float", { fg = p.yellow })
  hl("Identifier", { fg = p.fg })
  hl("Function", { fg = p.blue })
  hl("Statement", { fg = p.orange })
  hl("Conditional", { fg = p.orange })
  hl("Repeat", { fg = p.orange })
  hl("Label", { fg = p.red })
  hl("Operator", { fg = p.fg })
  hl("Keyword", { fg = p.orange })
  hl("Exception", { fg = p.orange })
  hl("PreProc", { fg = p.orange })
  hl("Include", { fg = p.orange })
  hl("Define", { fg = p.orange })
  hl("Macro", { fg = p.blue })
  hl("PreCondit", { fg = p.orange })
  hl("Type", { fg = p.yellow })
  hl("StorageClass", { fg = p.orange })
  hl("Structure", { fg = p.yellow })
  hl("Typedef", { fg = p.yellow })
  hl("Special", { fg = p.blue })
  hl("SpecialChar", { fg = p.cyan })
  hl("Tag", { fg = p.red })
  hl("Delimiter", { fg = p.fg })
  hl("SpecialComment", { fg = p.comment, italic = true })
  hl("Debug", { fg = p.red })
  hl("Underlined", { fg = p.blue, underline = true })
  hl("Ignore", { fg = p.comment })
  hl("Error", { fg = p.red, bold = true })
  hl("Todo", { fg = p.orange, bold = true })

  -- Treesitter
  hl("@variable", { fg = p.fg })
  hl("@variable.builtin", { fg = p.red })
  hl("@variable.parameter", { fg = p.fg })
  hl("@variable.member", { fg = p.red })
  hl("@constant", { fg = p.yellow })
  hl("@constant.builtin", { fg = p.yellow })
  hl("@constant.macro", { fg = p.yellow })
  hl("@module", { fg = p.fg })
  hl("@string", { fg = p.green })
  hl("@string.escape", { fg = p.cyan })
  hl("@string.regexp", { fg = p.cyan })
  hl("@character", { fg = p.green })
  hl("@number", { fg = p.yellow })
  hl("@boolean", { fg = p.yellow })
  hl("@float", { fg = p.yellow })
  hl("@function", { fg = p.blue })
  hl("@function.builtin", { fg = p.blue })
  hl("@function.method", { fg = p.blue })
  hl("@function.macro", { fg = p.blue })
  hl("@constructor", { fg = p.blue })
  hl("@keyword", { fg = p.orange })
  hl("@keyword.function", { fg = p.orange })
  hl("@keyword.operator", { fg = p.orange })
  hl("@keyword.return", { fg = p.orange })
  hl("@keyword.import", { fg = p.orange })
  hl("@keyword.conditional", { fg = p.orange })
  hl("@keyword.repeat", { fg = p.orange })
  hl("@keyword.exception", { fg = p.orange })
  hl("@operator", { fg = p.fg })
  hl("@punctuation.bracket", { fg = p.fg })
  hl("@punctuation.delimiter", { fg = p.fg })
  hl("@punctuation.special", { fg = p.cyan })
  hl("@type", { fg = p.yellow })
  hl("@type.builtin", { fg = p.yellow })
  hl("@type.qualifier", { fg = p.orange })
  hl("@attribute", { fg = p.yellow })
  hl("@tag", { fg = p.red })
  hl("@tag.attribute", { fg = p.yellow })
  hl("@tag.delimiter", { fg = p.fg })
  hl("@comment", { fg = p.comment, italic = true })
  hl("@markup.heading", { fg = p.red, bold = true })
  hl("@markup.list", { fg = p.red })
  hl("@markup.quote", { fg = p.green, italic = true })
  hl("@markup.strong", { fg = p.yellow, bold = true })
  hl("@markup.italic", { fg = p.orange, italic = true })
  hl("@markup.strikethrough", { strikethrough = true })
  hl("@markup.link.url", { fg = p.orange, underline = true })
  hl("@markup.link.label", { fg = p.blue })
  hl("@markup.raw", { fg = p.green })
  hl("@markup.raw.markdown_inline", { fg = p.green })

  -- YAML (treesitter specific)
  hl("@property.yaml", { fg = p.red })
  hl("@string.yaml", { fg = p.green })
  hl("@number.yaml", { fg = p.yellow })
  hl("@boolean.yaml", { fg = p.yellow })
  hl("@punctuation.delimiter.yaml", { fg = p.fg })
  hl("@punctuation.special.yaml", { fg = p.fg })
  hl("@label.yaml", { fg = p.blue })

  -- Comment annotations (all languages)
  hl("@comment.documentation", { fg = p.comment, italic = true })
  hl("@comment.error", { fg = p.red, bold = true })
  hl("@comment.warning", { fg = p.yellow })
  hl("@comment.todo", { fg = p.orange, bold = true })
  hl("@comment.note", { fg = p.blue })

  -- String special variants
  hl("@string.special.path", { fg = p.cyan })
  hl("@string.special.symbol", { fg = p.yellow })

  -- TypeScript/JSX/TSX
  hl("@tag.tsx", { fg = p.red })
  hl("@tag.javascript", { fg = p.red })
  hl("@tag.attribute.typescript", { fg = p.yellow })
  hl("@constructor.tsx", { fg = p.blue })

  -- Function calls (distinguish from definitions)
  hl("@function.call", { fg = p.blue })
  hl("@function.method.call", { fg = p.blue })

  -- Diff (treesitter)
  hl("@diff.plus", { fg = p.green })
  hl("@diff.minus", { fg = p.red })
  hl("@diff.delta", { fg = p.yellow })

  -- Type definition (Go, C, Rust)
  hl("@type.definition", { fg = p.yellow })

  -- Label (Nix, Lua, HCL)
  hl("@label", { fg = p.blue })

  -- LSP semantic tokens
  hl("@lsp.type.class", { fg = p.yellow })
  hl("@lsp.type.struct", { fg = p.yellow })
  hl("@lsp.type.enum", { fg = p.yellow })
  hl("@lsp.type.enumMember", { fg = p.yellow })
  hl("@lsp.type.interface", { fg = p.yellow })
  hl("@lsp.type.parameter", { fg = p.fg })
  hl("@lsp.type.property", { fg = p.red })
  hl("@lsp.type.variable", { fg = p.fg })
  hl("@lsp.type.keyword", { fg = p.orange })
  hl("@lsp.type.namespace", { fg = p.red })
  hl("@lsp.type.function", { fg = p.blue })
  hl("@lsp.type.method", { fg = p.blue })
  hl("@lsp.type.macro", { fg = p.blue })
  hl("@lsp.type.decorator", { fg = p.yellow })
  hl("@lsp.mod.deprecated", { strikethrough = true })

  -- LSP inlay hints
  hl("LspInlayHint", { fg = p.comment, italic = true })

  -- Diagnostics
  hl("DiagnosticError", { fg = p.red })
  hl("DiagnosticWarn", { fg = p.yellow })
  hl("DiagnosticInfo", { fg = p.blue })
  hl("DiagnosticHint", { fg = p.green })
  hl("DiagnosticUnderlineError", { undercurl = true, sp = p.red })
  hl("DiagnosticUnderlineWarn", { undercurl = true, sp = p.yellow })
  hl("DiagnosticUnderlineInfo", { undercurl = true, sp = p.blue })
  hl("DiagnosticUnderlineHint", { undercurl = true, sp = p.green })
  hl("DiagnosticVirtualTextError", { fg = p.red, italic = true })
  hl("DiagnosticVirtualTextWarn", { fg = p.yellow, italic = true })
  hl("DiagnosticVirtualTextInfo", { fg = p.blue, italic = true })
  hl("DiagnosticVirtualTextHint", { fg = p.green, italic = true })
  hl("DiagnosticUnnecessary", { link = "Comment" })
  hl("DiagnosticDeprecated", { strikethrough = true })

  -- Git signs
  hl("GitSignsAdd", { fg = p.green })
  hl("GitSignsChange", { fg = p.yellow })
  hl("GitSignsDelete", { fg = p.red })

  -- Which-key
  hl("WhichKey", { fg = p.orange })
  hl("WhichKeyGroup", { fg = p.blue })
  hl("WhichKeyDesc", { fg = p.fg })
  hl("WhichKeySeparator", { fg = p.comment })
  hl("WhichKeyFloat", { bg = p.bg_float })

  -- Blink.cmp
  hl("BlinkCmpMenu", { fg = p.fg, bg = p.bg_float })
  hl("BlinkCmpMenuBorder", { fg = p.border, bg = p.bg_float })
  hl("BlinkCmpMenuSelection", { bg = p.selection })
  hl("BlinkCmpLabel", { fg = p.fg })
  hl("BlinkCmpLabelMatch", { fg = p.blue, bold = true })
  hl("BlinkCmpKind", { fg = p.orange })
  hl("BlinkCmpGhostText", { fg = p.comment, italic = true })

  -- Snacks
  hl("SnacksPickerDir", { fg = p.comment })
  hl("SnacksPickerFile", { fg = p.fg })
  hl("SnacksPickerMatch", { fg = p.blue, bold = true })
  hl("SnacksPickerPrompt", { fg = p.blue })
  hl("SnacksPickerGitStatusAdded", { fg = p.green, bold = true })
  hl("SnacksPickerGitStatusUntracked", { fg = p.orange, bold = true })

  -- Indent guides
  hl("IblIndent", { fg = p.bg_alt })
  hl("IblScope", { fg = p.selection })
  hl("SnacksIndent", { fg = p.bg_alt })
  hl("SnacksIndentScope", { fg = p.selection })
end

return M
