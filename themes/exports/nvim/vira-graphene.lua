-- Auto-generated from themes/core/vira-graphene.yaml
-- Template: neovim-official-highlight-template v1
local M = {}

local p = {
  bg = "#212121",
  bg_float = "#212121",
  bg_alt = "#2b2b2b",
  border = "#3a3a3a",
  hover = "#2d2d2d",
  selection = "#414141",
  search = "#5f5136",
  search_active = "#6784c1",
  fg = "#D9D9D9",
  fg_muted = "#545454",
  fg_inactive = "#9A9A9A",
  fg_bright = "#D9D9D9",
  comment = "#545454",
  ghost_text = "#67789c",
  red = "#f07178",
  orange = "#f78c6c",
  yellow = "#FFCB6B",
  green = "#C3E88D",
  cyan = "#89DDFF",
  blue = "#82AAFF",
  magenta = "#C792EA",
  pink = "#FFCC00",
  linenr = "#8A8A8A",
  syntax_text = "#D9D9D9",
  syntax_comment = "#545454",
  syntax_string = "#C3E88D",
  syntax_string_escape = "#89DDFF",
  syntax_number = "#f78c6c",
  syntax_constant = "#f78c6c",
  syntax_keyword = "#C792EA",
  syntax_operator = "#89DDFF",
  syntax_variable = "#D9D9D9",
  syntax_parameter = "#D9D9D9",
  syntax_property = "#82AAFF",
  syntax_field = "#82AAFF",
  syntax_func = "#82AAFF",
  syntax_method = "#82AAFF",
  syntax_type = "#FFCB6B",
  syntax_class = "#FFCB6B",
  syntax_interface = "#FFCB6B",
  syntax_namespace = "#82AAFF",
  syntax_builtin = "#89DDFF",
  syntax_tag = "#f07178",
  syntax_attribute = "#FFCB6B",
  syntax_punctuation = "#545454",
  syntax_link = "#82AAFF",
  diff_add_bg = "#383d30",
  diff_change_bg = "#2f3440",
  diff_delete_bg = "#3e2c2d",
  diff_text_bg = "#524631",
  cursor = "#FFCC00",
  none = "NONE",
}

function M.setup()
  vim.cmd("hi clear")
  if vim.fn.exists("syntax_on") then
    vim.cmd("syntax reset")
  end
  vim.o.termguicolors = true
  vim.o.background = "dark"
  vim.g.colors_name = "vira_graphene"

  local hl = function(group, opts)
    vim.api.nvim_set_hl(0, group, opts)
  end

  -- UI
  hl("Normal", { fg = p.fg, bg = p.bg })
  hl("NormalNC", { fg = "#D9D9D9", bg = p.bg })
  hl("NormalFloat", { fg = p.fg, bg = p.bg_float })
  hl("FloatBorder", { fg = "#2E2E2E", bg = p.bg_float })
  hl("FloatTitle", { fg = "#80CBC4", bg = p.bg_float, bold = true })
  hl("FloatFooter", { fg = p.comment, bg = p.bg_float })
  hl("Cursor", { fg = p.cursor, reverse = true })
  hl("lCursor", { fg = p.cursor, reverse = true })
  hl("CursorIM", { fg = p.cursor, reverse = true })
  hl("TermCursor", { fg = p.cursor, reverse = true })
  hl("CursorLine", { bg = "NONE" })
  hl("CursorColumn", { bg = "NONE" })
  hl("ColorColumn", { bg = "#424242" })
  hl("LineNr", { fg = "#424242" })
  hl("LineNrAbove", { fg = "#424242" })
  hl("LineNrBelow", { fg = "#424242" })
  hl("CursorLineNr", { fg = "#676767", bold = false })
  hl("SignColumn", { bg = p.bg })
  hl("CursorLineSign", { fg = p.fg_bright, bg = "NONE" })
  hl("FoldColumn", { fg = "#424242", bg = p.bg })
  hl("CursorLineFold", { fg = p.fg_bright, bg = "NONE" })
  hl("WinSeparator", { fg = p.border })
  hl("VertSplit", { fg = p.border })
  hl("StatusLine", { fg = "#616161", bg = p.bg_float })
  hl("StatusLineNC", { fg = "#616161", bg = "#212121" })
  hl("StatusLineTerm", { fg = "#616161", bg = p.bg_float })
  hl("StatusLineTermNC", { fg = "#616161", bg = "#212121" })
  hl("TabLine", { fg = "#676767", bg = "#212121" })
  hl("TabLineFill", { bg = "#212121" })
  hl("TabLineSel", { fg = "#FFFFFF", bg = p.bg, bold = false })
  hl("WinBar", { fg = p.fg, bg = p.bg_float })
  hl("WinBarNC", { fg = "#676767", bg = "#212121" })
  hl("Pmenu", { fg = p.fg, bg = p.bg_float })
  hl("PmenuSel", { bg = "#2E2E2E" })
  hl("PmenuKind", { fg = p.orange, bg = p.bg_float })
  hl("PmenuKindSel", { fg = p.orange, bg = "#2E2E2E" })
  hl("PmenuExtra", { fg = p.comment, bg = p.bg_float })
  hl("PmenuExtraSel", { fg = p.comment, bg = "#2E2E2E" })
  hl("PmenuSbar", { bg = p.bg_alt })
  hl("PmenuThumb", { bg = p.border })
  hl("PmenuMatch", { fg = "#80CBC4", bg = p.bg_float, bold = true })
  hl("PmenuMatchSel", { fg = "#80CBC4", bg = "#2E2E2E", bold = true })
  hl("ComplMatchIns", { fg = p.blue, bold = true })
  hl("Visual", { bg = "#545454", fg = "#D9D9D9" })
  hl("VisualNOS", { bg = "#545454", fg = "#D9D9D9" })
  hl("Search", { bg = "#3C3C3C" })
  hl("CurSearch", { bg = "#3C3C3C", bold = false, sp = "#80CBC4", underline = true })
  hl("IncSearch", { bg = "#3C3C3C", sp = "#80CBC4", underline = true })
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
  hl("Comment", { fg = "#676767", italic = true })
  hl("Constant", { fg = "#D9D9D9" })
  hl("String", { fg = p.syntax_string })
  hl("Character", { fg = p.syntax_string })
  hl("Number", { fg = p.syntax_number })
  hl("Boolean", { fg = "#ff9cac" })
  hl("Float", { fg = p.syntax_number })
  hl("Identifier", { fg = p.syntax_variable })
  hl("Function", { fg = p.syntax_func })
  hl("Statement", { fg = "#89DDFF" })
  hl("Conditional", { fg = "#89DDFF", italic = true })
  hl("Repeat", { fg = "#89DDFF", italic = true })
  hl("Label", { fg = p.syntax_namespace })
  hl("Operator", { fg = p.syntax_operator })
  hl("Keyword", { fg = "#89DDFF" })
  hl("Exception", { fg = "#89DDFF", italic = true })
  hl("PreProc", { fg = "#89DDFF" })
  hl("Include", { fg = "#89DDFF", italic = true })
  hl("Define", { fg = "#89DDFF" })
  hl("Macro", { fg = p.syntax_builtin })
  hl("PreCondit", { fg = "#89DDFF", italic = true })
  hl("Type", { fg = p.syntax_type })
  hl("StorageClass", { fg = p.syntax_keyword })
  hl("Structure", { fg = p.syntax_class })
  hl("Typedef", { fg = p.syntax_type })
  hl("Special", { fg = p.syntax_builtin })
  hl("SpecialChar", { fg = "#D9D9D9" })
  hl("Tag", { fg = p.syntax_tag })
  hl("Delimiter", { fg = "#89DDFF" })
  hl("SpecialComment", { fg = "#676767", italic = true })
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
  hl("@variable.builtin", { fg = "#D9D9D9" })
  hl("@variable.parameter", { fg = p.syntax_parameter, italic = true })
  hl("@variable.parameter.builtin", { fg = "#D9D9D9", italic = true })
  hl("@variable.member", { fg = "#D9D9D9" })
  hl("@constant", { fg = "#D9D9D9" })
  hl("@constant.builtin", { fg = "#89DDFF" })
  hl("@constant.macro", { fg = p.syntax_builtin })
  hl("@module", { fg = "#FFCB6B" })
  hl("@module.builtin", { fg = "#FFCB6B" })
  hl("@label", { fg = p.syntax_namespace })
  hl("@string", { fg = p.syntax_string })
  hl("@string.documentation", { fg = p.syntax_string, italic = true })
  hl("@string.regexp", { fg = p.syntax_string_escape })
  hl("@string.escape", { fg = "#D9D9D9" })
  hl("@string.special", { fg = "#D9D9D9" })
  hl("@string.special.symbol", { fg = "#89DDFF" })
  hl("@string.special.path", { fg = p.syntax_string_escape })
  hl("@string.special.url", { fg = p.syntax_link, underline = true })
  hl("@character", { fg = p.syntax_string })
  hl("@character.special", { fg = "#D9D9D9" })
  hl("@boolean", { fg = "#ff9cac" })
  hl("@number", { fg = p.syntax_number })
  hl("@number.float", { fg = p.syntax_number })
  hl("@type", { fg = p.syntax_type })
  hl("@type.builtin", { fg = "#FFCB6B" })
  hl("@type.definition", { fg = p.syntax_class })
  hl("@attribute", { fg = p.syntax_attribute })
  hl("@attribute.builtin", { fg = p.syntax_builtin })
  hl("@property", { fg = "#D9D9D9" })
  hl("@function", { fg = p.syntax_func })
  hl("@function.builtin", { fg = "#82AAFF" })
  hl("@function.call", { fg = p.syntax_func })
  hl("@function.macro", { fg = p.syntax_builtin })
  hl("@function.method", { fg = p.syntax_method })
  hl("@function.method.call", { fg = p.syntax_method })
  hl("@constructor", { fg = "#82AAFF" })
  hl("@operator", { fg = p.syntax_operator })
  hl("@keyword", { fg = "#89DDFF" })
  hl("@keyword.coroutine", { fg = "#89DDFF", italic = true })
  hl("@keyword.function", { fg = p.syntax_keyword })
  hl("@keyword.operator", { fg = "#89DDFF" })
  hl("@keyword.import", { fg = "#89DDFF", italic = true })
  hl("@keyword.type", { fg = p.syntax_keyword })
  hl("@keyword.modifier", { fg = p.syntax_keyword })
  hl("@keyword.repeat", { fg = "#89DDFF", italic = true })
  hl("@keyword.return", { fg = "#89DDFF", italic = true })
  hl("@keyword.debug", { fg = "#89DDFF", italic = true })
  hl("@keyword.exception", { fg = "#89DDFF", italic = true })
  hl("@keyword.conditional", { fg = "#89DDFF", italic = true })
  hl("@keyword.conditional.ternary", { fg = "#89DDFF", italic = true })
  hl("@keyword.directive", { fg = "#89DDFF" })
  hl("@keyword.directive.define", { fg = "#89DDFF" })
  hl("@punctuation.delimiter", { fg = "#89DDFF" })
  hl("@punctuation.bracket", { fg = "#89DDFF" })
  hl("@punctuation.special", { fg = "#89DDFF" })
  hl("@comment", { fg = "#676767", italic = true })
  hl("@comment.documentation", { fg = "#676767", italic = true })
  hl("@comment.error", { fg = p.red, bold = true })
  hl("@comment.warning", { fg = p.yellow })
  hl("@comment.todo", { fg = p.orange, bold = true })
  hl("@comment.note", { fg = p.blue })
  hl("@markup.strong", { fg = "#f07178", bold = true })
  hl("@markup.italic", { fg = "#f07178", italic = true })
  hl("@markup.strikethrough", { strikethrough = true })
  hl("@markup.underline", { underline = true })
  hl("@markup.heading", { fg = "#89DDFF", bold = true })
  hl("@markup.heading.1", { fg = "#89DDFF", bold = true })
  hl("@markup.heading.2", { fg = "#89DDFF", bold = true })
  hl("@markup.heading.3", { fg = "#89DDFF", bold = true })
  hl("@markup.heading.4", { fg = "#89DDFF", bold = true })
  hl("@markup.heading.5", { fg = "#89DDFF", bold = true })
  hl("@markup.heading.6", { fg = "#89DDFF", bold = true })
  hl("@markup.quote", { fg = "#89DDFF", italic = true })
  hl("@markup.math", { fg = p.cyan })
  hl("@markup.link", { fg = p.blue })
  hl("@markup.link.label", { fg = "#f07178" })
  hl("@markup.link.url", { fg = p.orange, underline = true })
  hl("@markup.raw", { fg = p.green })
  hl("@markup.raw.block", { fg = p.green })
  hl("@markup.list", { fg = "#89DDFF" })
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
  hl("@lsp.type.class", { fg = "#82AAFF" })
  hl("@lsp.type.struct", { fg = p.syntax_type })
  hl("@lsp.type.enum", { fg = p.syntax_type })
  hl("@lsp.type.enumMember", { fg = "#D9D9D9" })
  hl("@lsp.type.interface", { fg = p.syntax_interface })
  hl("@lsp.type.parameter", { fg = p.syntax_parameter })
  hl("@lsp.type.property", { fg = "#D9D9D9" })
  hl("@lsp.type.variable", { fg = p.syntax_variable })
  hl("@lsp.type.keyword", { fg = "#89DDFF" })
  hl("@lsp.type.namespace", { fg = "#FFCB6B" })
  hl("@lsp.type.function", { fg = p.syntax_func })
  hl("@lsp.type.method", { fg = p.syntax_method })
  hl("@lsp.type.macro", { fg = p.syntax_builtin })
  hl("@lsp.type.decorator", { fg = p.syntax_attribute })
  hl("@lsp.mod.deprecated", { strikethrough = true })

  -- Override groups
  hl("BlinkCmpMenuBorder", { fg = "#2E2E2E" })
  hl("BlinkCmpMenuSelection", { bg = "#2E2E2E" })
  hl("BlinkCmpLabelMatch", { fg = "#80CBC4" })
  hl("@lsp.type.type", { fg = "#FFCB6B" })
  hl("@lsp.type.typeParameter", { fg = "#f78c6c" })
  hl("@lsp.type.comment", { fg = "#676767", italic = true })
  hl("@lsp.typemod.property.readonly", { fg = "#B2CCD6" })
  hl("@lsp.type.selfParameter", { fg = "#B2CCD6" })
  hl("@tag.attribute", { fg = "#C792EA" })
  hl("@tag.delimiter", { fg = "#89DDFF" })
  hl("SnacksPickerDir", { fg = "#676767" })
  hl("SnacksPickerPathIgnored", { fg = "#676767" })
  hl("SnacksPickerPathHidden", { fg = "#676767" })
  hl("SnacksPickerDimmed", { fg = "#676767" })
  hl("SnacksPickerMatch", { fg = "#80CBC4" })
  hl("SnacksPickerPrompt", { fg = "#80CBC4" })
  hl("SnacksIndent", { fg = "#383838" })
  hl("SnacksIndentScope", { fg = "#424242" })
  hl("IblIndent", { fg = "#383838" })
  hl("IblScope", { fg = "#424242" })

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
  hl("BlinkCmpMenuBorder", { fg = "#2E2E2E", bg = p.bg_float })
  hl("BlinkCmpMenuSelection", { bg = "#2E2E2E" })
  hl("BlinkCmpLabel", { fg = p.fg })
  hl("BlinkCmpLabelMatch", { fg = "#80CBC4", bold = true })
  hl("BlinkCmpKind", { fg = p.orange })
  hl("BlinkCmpGhostText", { fg = p.ghost_text, italic = true })
  hl("MinuetVirtualText", { link = "BlinkCmpGhostText" })

  -- Plugin: snacks.nvim
  hl("SnacksPickerDir", { fg = "#676767" })
  hl("SnacksPickerFile", { fg = p.fg })
  hl("SnacksPickerPathIgnored", { fg = "#676767" })
  hl("SnacksPickerPathHidden", { fg = "#676767" })
  hl("SnacksPickerDimmed", { fg = "#676767" })
  hl("SnacksPickerGitStatus", { fg = p.fg_inactive })
  hl("SnacksPickerGitStatusUntracked", { fg = p.fg_inactive })
  hl("SnacksPickerGitStatusIgnored", { fg = p.fg_inactive })
  hl("SnacksPickerMatch", { fg = "#80CBC4", bold = true })
  hl("SnacksPickerPrompt", { fg = "#80CBC4" })
  hl("SnacksIndent", { fg = "#383838" })
  hl("SnacksIndentScope", { fg = "#424242" })

  -- Plugin: indent-blankline / ibl
  hl("IblIndent", { fg = "#383838" })
  hl("IblScope", { fg = "#424242" })
end

return M
