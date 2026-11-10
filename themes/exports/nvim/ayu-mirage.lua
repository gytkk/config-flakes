-- Auto-generated from themes/core/ayu-mirage.yaml
-- Template: neovim-official-highlight-template v1
local M = {}

local p = {
  bg = "#242936",
  bg_float = "#1F2430",
  bg_alt = "#282E3B",
  border = "#171B24",
  hover = "#262b38",
  selection = "#3c8cdf",
  search = "#3a3b3d",
  search_active = "#387ec7",
  fg = "#CCCAC2",
  fg_muted = "#8A9199",
  fg_inactive = "#8A9199",
  fg_bright = "#8A9199",
  comment = "#B8CFE6",
  ghost_text = "#9bcff0",
  red = "#F28779",
  orange = "#F29E74",
  yellow = "#FFCC66",
  green = "#D5FF80",
  cyan = "#95E6CB",
  blue = "#73D0FF",
  magenta = "#DFBFFF",
  pink = "#FFAD66",
  linenr = "#8A9199",
  syntax_text = "#CCCAC2",
  syntax_comment = "#B8CFE6",
  syntax_string = "#D5FF80",
  syntax_string_escape = "#95E6CB",
  syntax_number = "#F29E74",
  syntax_constant = "#DFBFFF",
  syntax_keyword = "#FFAD66",
  syntax_operator = "#F29E74",
  syntax_variable = "#CCCAC2",
  syntax_parameter = "#DFBFFF",
  syntax_property = "#F28779",
  syntax_field = "#F28779",
  syntax_func = "#FFD173",
  syntax_method = "#FFD173",
  syntax_type = "#73D0FF",
  syntax_class = "#73D0FF",
  syntax_interface = "#73D0FF",
  syntax_namespace = "#73D0FF",
  syntax_builtin = "#95E6CB",
  syntax_tag = "#5CCFE6",
  syntax_attribute = "#FFD173",
  syntax_punctuation = "#CCCAC2",
  syntax_link = "#73D0FF",
  diff_add_bg = "#32423e",
  diff_change_bg = "#313e52",
  diff_delete_bg = "#413441",
  diff_text_bg = "#35373c",
  cursor = "#FFCC66",
  none = "NONE",
}

function M.setup()
  vim.cmd("hi clear")
  if vim.fn.exists("syntax_on") then
    vim.cmd("syntax reset")
  end
  vim.o.termguicolors = true
  vim.o.background = "dark"
  vim.g.colors_name = "ayu_mirage"

  local hl = function(group, opts)
    vim.api.nvim_set_hl(0, group, opts)
  end

  -- UI
  hl("Normal", { fg = p.fg, bg = p.bg })
  hl("NormalNC", { fg = p.fg_muted, bg = p.bg })
  hl("NormalFloat", { fg = p.fg, bg = p.bg_float })
  hl("FloatBorder", { fg = p.border, bg = p.bg_float })
  hl("FloatTitle", { fg = p.blue, bg = p.bg_float, bold = true })
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
  hl("PmenuSel", { bg = p.selection })
  hl("PmenuKind", { fg = p.orange, bg = p.bg_float })
  hl("PmenuKindSel", { fg = p.orange, bg = p.selection })
  hl("PmenuExtra", { fg = p.comment, bg = p.bg_float })
  hl("PmenuExtraSel", { fg = p.comment, bg = p.selection })
  hl("PmenuSbar", { bg = p.bg_alt })
  hl("PmenuThumb", { bg = p.border })
  hl("PmenuMatch", { fg = p.blue, bg = p.bg_float, bold = true })
  hl("PmenuMatchSel", { fg = p.blue, bg = p.selection, bold = true })
  hl("ComplMatchIns", { fg = p.blue, bold = true })
  hl("Visual", { bg = p.selection })
  hl("VisualNOS", { bg = p.selection })
  hl("Search", { bg = p.search })
  hl("CurSearch", { bg = p.search_active, bold = true })
  hl("IncSearch", { bg = p.search_active })
  hl("Substitute", { fg = p.fg, bg = p.search_active, bold = true })
  hl("MatchParen", { fg = p.blue, underline = true })
  hl("QuickFixLine", { bg = p.hover })
  hl("Folded", { fg = p.comment, bg = p.bg_float })
  hl("Directory", { fg = p.blue })
  hl("Title", { fg = p.blue, bold = true })
  hl("Question", { fg = p.green })
  hl("MoreMsg", { fg = p.green })
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
  hl("Constant", { fg = p.syntax_constant })
  hl("String", { fg = p.syntax_string })
  hl("Character", { fg = p.syntax_string })
  hl("Number", { fg = p.syntax_number })
  hl("Boolean", { fg = p.syntax_constant })
  hl("Float", { fg = p.syntax_number })
  hl("Identifier", { fg = p.syntax_variable })
  hl("Function", { fg = p.syntax_func })
  hl("Statement", { fg = p.syntax_keyword })
  hl("Conditional", { fg = p.syntax_keyword })
  hl("Repeat", { fg = p.syntax_keyword })
  hl("Label", { fg = p.syntax_namespace })
  hl("Operator", { fg = p.syntax_operator })
  hl("Keyword", { fg = p.syntax_keyword })
  hl("Exception", { fg = p.syntax_keyword })
  hl("PreProc", { fg = p.syntax_keyword })
  hl("Include", { fg = p.syntax_keyword })
  hl("Define", { fg = p.syntax_keyword })
  hl("Macro", { fg = p.syntax_builtin })
  hl("PreCondit", { fg = p.syntax_keyword })
  hl("Type", { fg = p.syntax_type })
  hl("StorageClass", { fg = p.syntax_keyword })
  hl("Structure", { fg = p.syntax_class })
  hl("Typedef", { fg = p.syntax_type })
  hl("Special", { fg = p.syntax_builtin })
  hl("SpecialChar", { fg = p.syntax_string_escape })
  hl("Tag", { fg = p.syntax_tag })
  hl("Delimiter", { fg = p.syntax_punctuation })
  hl("SpecialComment", { fg = p.comment, italic = true })
  hl("Debug", { fg = p.red })
  hl("Underlined", { fg = p.syntax_link, underline = true })
  hl("Ignore", { fg = p.comment })
  hl("Error", { fg = p.red, bold = true })
  hl("Todo", { fg = p.orange, bold = true })
  hl("Added", { fg = p.green })
  hl("Changed", { fg = p.yellow })
  hl("Removed", { fg = p.red })

  -- Treesitter standard captures
  hl("@variable", { fg = p.syntax_variable })
  hl("@variable.builtin", { fg = p.syntax_builtin })
  hl("@variable.parameter", { fg = p.syntax_parameter })
  hl("@variable.parameter.builtin", { fg = p.syntax_builtin })
  hl("@variable.member", { fg = p.syntax_field })
  hl("@constant", { fg = p.syntax_constant })
  hl("@constant.builtin", { fg = p.syntax_builtin })
  hl("@constant.macro", { fg = p.syntax_builtin })
  hl("@module", { fg = p.syntax_namespace })
  hl("@module.builtin", { fg = p.syntax_builtin })
  hl("@label", { fg = p.syntax_namespace })
  hl("@string", { fg = p.syntax_string })
  hl("@string.documentation", { fg = p.syntax_string, italic = true })
  hl("@string.regexp", { fg = p.syntax_string_escape })
  hl("@string.escape", { fg = p.syntax_string_escape })
  hl("@string.special", { fg = p.syntax_string_escape })
  hl("@string.special.symbol", { fg = p.syntax_constant })
  hl("@string.special.path", { fg = p.syntax_string_escape })
  hl("@string.special.url", { fg = p.syntax_link, underline = true })
  hl("@character", { fg = p.syntax_string })
  hl("@character.special", { fg = p.syntax_string_escape })
  hl("@boolean", { fg = p.syntax_constant })
  hl("@number", { fg = p.syntax_number })
  hl("@number.float", { fg = p.syntax_number })
  hl("@type", { fg = p.syntax_type })
  hl("@type.builtin", { fg = p.syntax_builtin })
  hl("@type.definition", { fg = p.syntax_class })
  hl("@attribute", { fg = p.syntax_attribute })
  hl("@attribute.builtin", { fg = p.syntax_builtin })
  hl("@property", { fg = p.syntax_property })
  hl("@function", { fg = p.syntax_func })
  hl("@function.builtin", { fg = p.syntax_builtin })
  hl("@function.call", { fg = p.syntax_func })
  hl("@function.macro", { fg = p.syntax_builtin })
  hl("@function.method", { fg = p.syntax_method })
  hl("@function.method.call", { fg = p.syntax_method })
  hl("@constructor", { fg = p.syntax_class })
  hl("@operator", { fg = p.syntax_operator })
  hl("@keyword", { fg = p.syntax_keyword })
  hl("@keyword.coroutine", { fg = p.syntax_keyword })
  hl("@keyword.function", { fg = p.syntax_keyword })
  hl("@keyword.operator", { fg = p.syntax_keyword })
  hl("@keyword.import", { fg = p.syntax_keyword })
  hl("@keyword.type", { fg = p.syntax_keyword })
  hl("@keyword.modifier", { fg = p.syntax_keyword })
  hl("@keyword.repeat", { fg = p.syntax_keyword })
  hl("@keyword.return", { fg = p.syntax_keyword })
  hl("@keyword.debug", { fg = p.syntax_keyword })
  hl("@keyword.exception", { fg = p.syntax_keyword })
  hl("@keyword.conditional", { fg = p.syntax_keyword })
  hl("@keyword.conditional.ternary", { fg = p.syntax_keyword })
  hl("@keyword.directive", { fg = p.syntax_keyword })
  hl("@keyword.directive.define", { fg = p.syntax_keyword })
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
  hl("@lsp.type.class", { fg = p.syntax_class })
  hl("@lsp.type.struct", { fg = p.syntax_type })
  hl("@lsp.type.enum", { fg = p.syntax_type })
  hl("@lsp.type.enumMember", { fg = p.syntax_constant })
  hl("@lsp.type.interface", { fg = p.syntax_interface })
  hl("@lsp.type.parameter", { fg = p.syntax_parameter })
  hl("@lsp.type.property", { fg = p.syntax_property })
  hl("@lsp.type.variable", { fg = p.syntax_variable })
  hl("@lsp.type.keyword", { fg = p.syntax_keyword })
  hl("@lsp.type.namespace", { fg = p.syntax_namespace })
  hl("@lsp.type.function", { fg = p.syntax_func })
  hl("@lsp.type.method", { fg = p.syntax_method })
  hl("@lsp.type.macro", { fg = p.syntax_builtin })
  hl("@lsp.type.decorator", { fg = p.syntax_attribute })
  hl("@lsp.mod.deprecated", { strikethrough = true })

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
