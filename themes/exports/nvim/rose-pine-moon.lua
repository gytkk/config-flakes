-- Auto-generated from themes/core/rose-pine-moon.yaml
-- Template: neovim-official-highlight-template v1
local M = {}

local p = {
  bg = "#232136",
  bg_float = "#2a273f",
  bg_alt = "#393552",
  border = "#56526e",
  hover = "#3d3956",
  selection = "#3f3c54",
  search = "#5b4351",
  search_active = "#9781b5",
  fg = "#e0def4",
  fg_muted = "#908caa",
  fg_inactive = "#908caa",
  fg_bright = "#e0def4",
  comment = "#6e6a86",
  ghost_text = "#9284af",
  red = "#eb6f92",
  orange = "#f6c177",
  yellow = "#ea9a97",
  green = "#3e8fb0",
  cyan = "#9ccfd8",
  blue = "#c4a7e7",
  magenta = "#f6c177",
  pink = "#eb6f92",
  linenr = "#908caa",
  syntax_text = "#e0def4",
  syntax_comment = "#6e6a86",
  syntax_string = "#ea9a97",
  syntax_string_escape = "#9ccfd8",
  syntax_number = "#f6c177",
  syntax_constant = "#f6c177",
  syntax_keyword = "#eb6f92",
  syntax_operator = "#908caa",
  syntax_variable = "#e0def4",
  syntax_parameter = "#c4a7e7",
  syntax_property = "#9ccfd8",
  syntax_field = "#9ccfd8",
  syntax_func = "#3e8fb0",
  syntax_method = "#3e8fb0",
  syntax_type = "#f6c177",
  syntax_class = "#f6c177",
  syntax_interface = "#f6c177",
  syntax_namespace = "#c4a7e7",
  syntax_builtin = "#eb6f92",
  syntax_tag = "#eb6f92",
  syntax_attribute = "#f6c177",
  syntax_punctuation = "#908caa",
  syntax_link = "#c4a7e7",
  diff_add_bg = "#34394d",
  diff_change_bg = "#41373f",
  diff_delete_bg = "#3f2c43",
  diff_text_bg = "#4f3c4b",
  cursor = "#908caa",
  none = "NONE",
}

function M.setup()
  vim.cmd("hi clear")
  if vim.fn.exists("syntax_on") then
    vim.cmd("syntax reset")
  end
  vim.o.termguicolors = true
  vim.o.background = "dark"
  vim.g.colors_name = "rose_pine_moon"

  local hl = function(group, opts)
    vim.api.nvim_set_hl(0, group, opts)
  end

  -- UI
  hl("Normal", { fg = p.fg, bg = p.bg })
  hl("NormalNC", { fg = p.fg_muted, bg = p.bg })
  hl("NormalFloat", { fg = p.fg, bg = p.bg_float })
  hl("FloatBorder", { fg = p.border, bg = p.bg_float })
  hl("FloatTitle", { fg = "#9ccfd8", bg = p.bg_float, bold = true })
  hl("FloatFooter", { fg = p.comment, bg = p.bg_float })
  hl("Cursor", { fg = p.cursor, reverse = true })
  hl("lCursor", { fg = p.cursor, reverse = true })
  hl("CursorIM", { fg = p.cursor, reverse = true })
  hl("TermCursor", { fg = p.cursor, reverse = true })
  hl("CursorLine", { bg = p.bg_alt })
  hl("CursorColumn", { bg = p.bg_alt })
  hl("ColorColumn", { bg = p.bg_alt })
  hl("LineNr", { fg = p.linenr })
  hl("LineNrAbove", { fg = p.linenr })
  hl("LineNrBelow", { fg = p.linenr })
  hl("CursorLineNr", { fg = p.fg_bright, bold = true })
  hl("SignColumn", { bg = p.bg })
  hl("CursorLineSign", { fg = p.fg_bright, bg = p.bg_alt })
  hl("FoldColumn", { fg = p.linenr, bg = p.bg })
  hl("CursorLineFold", { fg = p.fg_bright, bg = p.bg_alt })
  hl("WinSeparator", { fg = p.border })
  hl("VertSplit", { fg = p.border })
  hl("StatusLine", { fg = p.fg, bg = p.bg_float })
  hl("StatusLineNC", { fg = p.fg_muted, bg = p.bg_alt })
  hl("StatusLineTerm", { fg = p.fg, bg = p.bg_float })
  hl("StatusLineTermNC", { fg = p.fg_muted, bg = p.bg_alt })
  hl("TabLine", { fg = p.fg_muted, bg = p.bg_alt })
  hl("TabLineFill", { bg = p.bg_alt })
  hl("TabLineSel", { fg = p.fg, bg = p.bg, bold = true })
  hl("WinBar", { fg = p.fg, bg = p.bg_float })
  hl("WinBarNC", { fg = p.fg_muted, bg = p.bg_alt })
  hl("Pmenu", { fg = p.fg, bg = p.bg_float })
  hl("PmenuSel", { bg = "#393552", fg = "#e0def4" })
  hl("PmenuKind", { fg = p.orange, bg = p.bg_float })
  hl("PmenuKindSel", { fg = p.orange, bg = p.selection })
  hl("PmenuExtra", { fg = p.comment, bg = p.bg_float })
  hl("PmenuExtraSel", { fg = p.comment, bg = p.selection })
  hl("PmenuSbar", { bg = p.bg_alt })
  hl("PmenuThumb", { bg = p.border })
  hl("PmenuMatch", { fg = p.blue, bg = p.bg_float, bold = true })
  hl("PmenuMatchSel", { fg = p.blue, bg = p.selection, bold = true })
  hl("ComplMatchIns", { fg = p.blue, bold = true })
  hl("Visual", { bg = "#c4a7e7" })
  hl("VisualNOS", { bg = p.selection })
  hl("Search", { bg = "#f6c177", fg = "#e0def4" })
  hl("CurSearch", { bg = "#f6c177", bold = true, fg = "#232136" })
  hl("IncSearch", { bg = p.search_active, link = "CurSearch" })
  hl("Substitute", { fg = p.fg, bg = p.search_active, bold = true })
  hl("MatchParen", { fg = "#3e8fb0", underline = false, bg = "#3e8fb0" })
  hl("QuickFixLine", { bg = p.hover })
  hl("Folded", { fg = p.comment, bg = p.bg_float })
  hl("Directory", { fg = p.blue })
  hl("Title", { fg = p.blue, bold = true })
  hl("Question", { fg = p.green })
  hl("MoreMsg", { fg = "#c4a7e7" })
  hl("ModeMsg", { fg = p.fg, bold = true })
  hl("MsgArea", { fg = p.fg, bg = p.bg })
  hl("MsgSeparator", { fg = p.border, bg = p.bg })
  hl("WarningMsg", { fg = p.yellow, bold = true })
  hl("ErrorMsg", { fg = p.red, bold = true })
  hl("NonText", { fg = p.border })
  hl("EndOfBuffer", { fg = p.bg_float })
  hl("SpecialKey", { fg = p.border })
  hl("Whitespace", { fg = p.border })
  hl("Conceal", { fg = p.comment })
  hl("WildMenu", { bg = p.selection })
  hl("SnippetTabstop", { bg = p.selection })
  hl("SpellBad", { undercurl = true, sp = p.red })
  hl("SpellCap", { undercurl = true, sp = p.yellow })
  hl("SpellRare", { undercurl = true, sp = p.orange })
  hl("SpellLocal", { undercurl = true, sp = p.cyan })
  hl("DiffAdd", { bg = p.diff_add_bg })
  hl("DiffChange", { bg = p.diff_change_bg })
  hl("DiffDelete", { bg = p.diff_delete_bg })
  hl("DiffText", { bg = p.diff_text_bg, bold = true })

  -- Legacy syntax groups
  hl("Comment", { fg = p.comment, italic = true })
  hl("Constant", { fg = "#f6c177" })
  hl("String", { fg = p.syntax_string })
  hl("Character", { fg = p.syntax_string })
  hl("Number", { fg = "#f6c177" })
  hl("Boolean", { fg = "#ea9a97" })
  hl("Float", { fg = "#f6c177" })
  hl("Identifier", { fg = "#e0def4" })
  hl("Function", { fg = "#ea9a97" })
  hl("Statement", { fg = "#3e8fb0", bold = true })
  hl("Conditional", { fg = "#3e8fb0" })
  hl("Repeat", { fg = "#3e8fb0" })
  hl("Label", { fg = "#9ccfd8" })
  hl("Operator", { fg = p.syntax_operator })
  hl("Keyword", { fg = "#3e8fb0" })
  hl("Exception", { fg = "#3e8fb0" })
  hl("PreProc", { fg = "#c4a7e7" })
  hl("Include", { fg = "#c4a7e7" })
  hl("Define", { fg = "#c4a7e7" })
  hl("Macro", { fg = "#c4a7e7" })
  hl("PreCondit", { fg = "#c4a7e7" })
  hl("Type", { fg = "#9ccfd8" })
  hl("StorageClass", { fg = "#9ccfd8" })
  hl("Structure", { fg = "#9ccfd8" })
  hl("Typedef", { fg = p.syntax_type, link = "Type" })
  hl("Special", { fg = "#9ccfd8" })
  hl("SpecialChar", { fg = p.syntax_string_escape, link = "Special" })
  hl("Tag", { fg = "#9ccfd8" })
  hl("Delimiter", { fg = p.syntax_punctuation })
  hl("SpecialComment", { fg = "#c4a7e7", italic = true })
  hl("Debug", { fg = p.red })
  hl("Underlined", { fg = p.syntax_link, underline = true })
  hl("Ignore", { fg = p.comment })
  hl("Error", { fg = p.red, bold = true })
  hl("Todo", { fg = "#ea9a97", bold = true, bg = "#ea9a97" })
  hl("Added", { fg = p.green })
  hl("Changed", { fg = p.yellow })
  hl("Removed", { fg = p.red })

  -- Treesitter standard captures
  hl("@variable", { fg = p.syntax_variable })
  hl("@variable.builtin", { fg = "#c4a7e7" })
  hl("@variable.parameter", { fg = p.syntax_parameter })
  hl("@variable.parameter.builtin", { fg = p.syntax_builtin })
  hl("@variable.member", { fg = p.syntax_field })
  hl("@constant", { fg = "#f6c177" })
  hl("@constant.builtin", { fg = "#c4a7e7" })
  hl("@constant.macro", { fg = "#c4a7e7" })
  hl("@module", { fg = p.syntax_namespace })
  hl("@module.builtin", { fg = "#c4a7e7" })
  hl("@label", { fg = p.syntax_namespace })
  hl("@string", { fg = p.syntax_string })
  hl("@string.documentation", { fg = p.syntax_string, italic = true })
  hl("@string.regexp", { fg = p.syntax_string_escape })
  hl("@string.escape", { fg = "#ea9a97" })
  hl("@string.special", { fg = p.syntax_string_escape })
  hl("@string.special.symbol", { fg = p.syntax_constant })
  hl("@string.special.path", { fg = p.syntax_string_escape })
  hl("@string.special.url", { fg = p.syntax_link, underline = true })
  hl("@character", { fg = p.syntax_string })
  hl("@character.special", { fg = p.syntax_string_escape })
  hl("@boolean", { fg = "#ea9a97" })
  hl("@number", { fg = "#f6c177" })
  hl("@number.float", { fg = "#f6c177" })
  hl("@type", { fg = "#9ccfd8" })
  hl("@type.builtin", { fg = "#c4a7e7" })
  hl("@type.definition", { fg = "#9ccfd8" })
  hl("@attribute", { fg = "#c4a7e7" })
  hl("@attribute.builtin", { fg = "#c4a7e7" })
  hl("@property", { fg = p.syntax_property })
  hl("@function", { fg = "#ea9a97" })
  hl("@function.builtin", { fg = "#c4a7e7" })
  hl("@function.call", { fg = "#ea9a97" })
  hl("@function.macro", { fg = "#c4a7e7" })
  hl("@function.method", { fg = "#ea9a97" })
  hl("@function.method.call", { fg = "#ea9a97" })
  hl("@constructor", { link = "@type" })
  hl("@operator", { fg = p.syntax_operator })
  hl("@keyword", { fg = "#3e8fb0" })
  hl("@keyword.coroutine", { fg = "#3e8fb0" })
  hl("@keyword.function", { fg = "#3e8fb0" })
  hl("@keyword.operator", { fg = "#3e8fb0" })
  hl("@keyword.import", { fg = "#3e8fb0" })
  hl("@keyword.type", { fg = "#3e8fb0" })
  hl("@keyword.modifier", { fg = "#3e8fb0" })
  hl("@keyword.repeat", { fg = "#3e8fb0" })
  hl("@keyword.return", { fg = "#3e8fb0" })
  hl("@keyword.debug", { fg = "#3e8fb0" })
  hl("@keyword.exception", { fg = "#3e8fb0" })
  hl("@keyword.conditional", { fg = "#3e8fb0" })
  hl("@keyword.conditional.ternary", { fg = "#3e8fb0" })
  hl("@keyword.directive", { fg = "#c4a7e7" })
  hl("@keyword.directive.define", { fg = "#c4a7e7" })
  hl("@punctuation.delimiter", { fg = p.syntax_punctuation })
  hl("@punctuation.bracket", { fg = p.syntax_punctuation })
  hl("@punctuation.special", { fg = p.syntax_punctuation })
  hl("@comment", { fg = p.comment, italic = true })
  hl("@comment.documentation", { fg = p.comment, italic = true })
  hl("@comment.error", { fg = p.red, bold = true })
  hl("@comment.warning", { fg = p.yellow })
  hl("@comment.todo", { fg = p.orange, bold = true })
  hl("@comment.note", { fg = p.blue })
  hl("@markup.strong", { fg = p.yellow, bold = true })
  hl("@markup.italic", { fg = p.orange, italic = true })
  hl("@markup.strikethrough", { strikethrough = true })
  hl("@markup.underline", { underline = true })
  hl("@markup.heading", { fg = p.red, bold = true })
  hl("@markup.heading.1", { fg = p.red, bold = true })
  hl("@markup.heading.2", { fg = p.orange, bold = true })
  hl("@markup.heading.3", { fg = p.yellow, bold = true })
  hl("@markup.heading.4", { fg = p.green, bold = true })
  hl("@markup.heading.5", { fg = p.blue, bold = true })
  hl("@markup.heading.6", { fg = p.magenta, bold = true })
  hl("@markup.quote", { fg = p.green, italic = true })
  hl("@markup.math", { fg = p.cyan })
  hl("@markup.link", { fg = p.blue })
  hl("@markup.link.label", { fg = p.blue })
  hl("@markup.link.url", { fg = p.orange, underline = true })
  hl("@markup.raw", { fg = p.green })
  hl("@markup.raw.block", { fg = p.green })
  hl("@markup.list", { fg = p.red })
  hl("@markup.list.checked", { fg = p.green })
  hl("@markup.list.unchecked", { fg = p.comment })
  hl("@diff.plus", { fg = p.green })
  hl("@diff.minus", { fg = p.red })
  hl("@diff.delta", { fg = p.yellow })

  -- Diagnostics
  hl("DiagnosticError", { fg = p.red })
  hl("DiagnosticWarn", { fg = p.yellow })
  hl("DiagnosticInfo", { fg = p.blue })
  hl("DiagnosticHint", { fg = p.cyan })
  hl("DiagnosticOk", { fg = p.green })
  hl("DiagnosticVirtualTextError", { fg = p.red, italic = true })
  hl("DiagnosticVirtualTextWarn", { fg = p.yellow, italic = true })
  hl("DiagnosticVirtualTextInfo", { fg = p.blue, italic = true })
  hl("DiagnosticVirtualTextHint", { fg = p.cyan, italic = true })
  hl("DiagnosticVirtualTextOk", { fg = p.green, italic = true })
  hl("DiagnosticVirtualLinesError", { fg = p.red, italic = true })
  hl("DiagnosticVirtualLinesWarn", { fg = p.yellow, italic = true })
  hl("DiagnosticVirtualLinesInfo", { fg = p.blue, italic = true })
  hl("DiagnosticVirtualLinesHint", { fg = p.cyan, italic = true })
  hl("DiagnosticVirtualLinesOk", { fg = p.green, italic = true })
  hl("DiagnosticUnderlineError", { undercurl = true, sp = p.red })
  hl("DiagnosticUnderlineWarn", { undercurl = true, sp = p.yellow })
  hl("DiagnosticUnderlineInfo", { undercurl = true, sp = p.blue })
  hl("DiagnosticUnderlineHint", { undercurl = true, sp = p.cyan })
  hl("DiagnosticUnderlineOk", { undercurl = true, sp = p.green })
  hl("DiagnosticFloatingError", { fg = p.red })
  hl("DiagnosticFloatingWarn", { fg = p.yellow })
  hl("DiagnosticFloatingInfo", { fg = p.blue })
  hl("DiagnosticFloatingHint", { fg = p.cyan })
  hl("DiagnosticFloatingOk", { fg = p.green })
  hl("DiagnosticSignError", { fg = p.red })
  hl("DiagnosticSignWarn", { fg = p.yellow })
  hl("DiagnosticSignInfo", { fg = p.blue })
  hl("DiagnosticSignHint", { fg = p.cyan })
  hl("DiagnosticSignOk", { fg = p.green })
  hl("DiagnosticDeprecated", { strikethrough = true })
  hl("DiagnosticUnnecessary", { link = "Comment" })

  -- LSP
  hl("LspReferenceText", { bg = p.selection })
  hl("LspReferenceRead", { bg = p.selection })
  hl("LspReferenceWrite", { bg = p.selection })
  hl("LspReferenceTarget", { bg = p.hover })
  hl("LspInlayHint", { fg = p.comment, italic = true })
  hl("LspCodeLens", { fg = p.comment, italic = true })
  hl("LspCodeLensSeparator", { fg = p.border })
  hl("LspSignatureActiveParameter", { fg = p.syntax_parameter, underline = true })
  hl("@lsp.type.class", { fg = "#9ccfd8" })
  hl("@lsp.type.struct", { fg = "#9ccfd8" })
  hl("@lsp.type.enum", { fg = "#9ccfd8" })
  hl("@lsp.type.enumMember", { fg = "#f6c177" })
  hl("@lsp.type.interface", { fg = "#9ccfd8" })
  hl("@lsp.type.parameter", { fg = p.syntax_parameter })
  hl("@lsp.type.property", { fg = p.syntax_property })
  hl("@lsp.type.variable", { fg = p.syntax_variable })
  hl("@lsp.type.keyword", { fg = "#3e8fb0" })
  hl("@lsp.type.namespace", { fg = p.syntax_namespace })
  hl("@lsp.type.function", { fg = "#ea9a97" })
  hl("@lsp.type.method", { fg = "#ea9a97" })
  hl("@lsp.type.macro", { fg = "#c4a7e7" })
  hl("@lsp.type.decorator", { fg = "#c4a7e7" })
  hl("@lsp.mod.deprecated", { strikethrough = true })

  -- Override groups
  hl("@tag", { fg = "#9ccfd8" })

  -- Plugin: GitSigns
  hl("GitSignsAdd", { fg = p.green })
  hl("GitSignsChange", { fg = p.yellow })
  hl("GitSignsDelete", { fg = p.red })

  -- Plugin: which-key
  hl("WhichKey", { fg = p.orange })
  hl("WhichKeyGroup", { fg = p.blue })
  hl("WhichKeyDesc", { fg = p.fg })
  hl("WhichKeySeparator", { fg = p.comment })
  hl("WhichKeyFloat", { bg = p.bg_float })

  -- Plugin: blink.cmp
  hl("BlinkCmpMenu", { fg = p.fg, bg = p.bg_float })
  hl("BlinkCmpMenuBorder", { fg = p.border, bg = p.bg_float })
  hl("BlinkCmpMenuSelection", { bg = p.selection })
  hl("BlinkCmpLabel", { fg = p.fg })
  hl("BlinkCmpLabelMatch", { fg = p.blue, bold = true })
  hl("BlinkCmpKind", { fg = p.orange })
  hl("BlinkCmpGhostText", { fg = p.ghost_text, italic = true })
  hl("MinuetVirtualText", { link = "BlinkCmpGhostText" })

  -- Plugin: snacks.nvim
  hl("SnacksPickerDir", { fg = p.fg_inactive })
  hl("SnacksPickerFile", { fg = p.fg })
  hl("SnacksPickerPathIgnored", { fg = p.fg_inactive })
  hl("SnacksPickerPathHidden", { fg = p.fg_inactive })
  hl("SnacksPickerDimmed", { fg = p.fg_inactive })
  hl("SnacksPickerGitStatus", { fg = p.fg_inactive })
  hl("SnacksPickerGitStatusUntracked", { fg = p.fg_inactive })
  hl("SnacksPickerGitStatusIgnored", { fg = p.fg_inactive })
  hl("SnacksPickerMatch", { fg = p.blue, bold = true })
  hl("SnacksPickerPrompt", { fg = p.blue })
  hl("SnacksIndent", { fg = p.bg_alt })
  hl("SnacksIndentScope", { fg = p.selection })

  -- Plugin: indent-blankline / ibl
  hl("IblIndent", { fg = p.bg_alt })
  hl("IblScope", { fg = p.selection })
end

return M
