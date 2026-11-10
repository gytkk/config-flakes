-- Auto-generated from themes/core/monokai-pro-classic.yaml
-- Template: neovim-official-highlight-template v1
local M = {}

local p = {
  bg = "#272822",
  bg_float = "#1d1e19",
  bg_alt = "#3b3c35",
  border = "#57584f",
  hover = "#3f4039",
  selection = "#383932",
  search = "#5c5a39",
  search_active = "#5a5c53",
  fg = "#fdfff1",
  fg_muted = "#6e7066",
  fg_inactive = "#939284",
  fg_bright = "#c0c1b5",
  comment = "#6e7066",
  ghost_text = "#729d9d",
  red = "#f92672",
  orange = "#fd971f",
  yellow = "#e6db74",
  green = "#a6e22e",
  cyan = "#66d9ef",
  blue = "#78dce8",
  magenta = "#ae81ff",
  pink = "#f82570",
  linenr = "#6e7066",
  syntax_text = "#fdfff1",
  syntax_comment = "#6e7066",
  syntax_string = "#e6db74",
  syntax_string_escape = "#fd971f",
  syntax_number = "#ae81ff",
  syntax_constant = "#ae81ff",
  syntax_keyword = "#f92672",
  syntax_operator = "#fdfff1",
  syntax_variable = "#fdfff1",
  syntax_parameter = "#fd971f",
  syntax_property = "#fdfff1",
  syntax_field = "#fdfff1",
  syntax_func = "#a6e22e",
  syntax_method = "#a6e22e",
  syntax_type = "#66d9ef",
  syntax_class = "#66d9ef",
  syntax_interface = "#66d9ef",
  syntax_namespace = "#f92672",
  syntax_builtin = "#66d9ef",
  syntax_tag = "#f92672",
  syntax_attribute = "#a6e22e",
  syntax_punctuation = "#fdfff1",
  syntax_link = "#819aff",
  diff_add_bg = "#394224",
  diff_change_bg = "#453822",
  diff_delete_bg = "#44282d",
  diff_text_bg = "#514f34",
  cursor = "#fdfff1",
  none = "NONE",
}

function M.setup()
  vim.cmd("hi clear")
  if vim.fn.exists("syntax_on") then
    vim.cmd("syntax reset")
  end
  vim.o.termguicolors = true
  vim.o.background = "dark"
  vim.g.colors_name = "monokai_pro_classic"

  local hl = function(group, opts)
    vim.api.nvim_set_hl(0, group, opts)
  end

  -- UI
  hl("Normal", { fg = p.fg, bg = p.bg })
  hl("NormalNC", { fg = p.fg_muted, bg = p.bg })
  hl("NormalFloat", { fg = p.fg, bg = "#3b3c35" })
  hl("FloatBorder", { fg = "#6e7066", bg = "#3b3c35" })
  hl("FloatTitle", { fg = "#f92672", bg = "#3b3c35", bold = true })
  hl("FloatFooter", { fg = p.comment, bg = "#3b3c35" })
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
  hl("StatusLine", { fg = p.fg, bg = "#3b3c35" })
  hl("StatusLineNC", { fg = p.fg_muted, bg = "#3b3c35" })
  hl("StatusLineTerm", { fg = p.fg, bg = "#3b3c35" })
  hl("StatusLineTermNC", { fg = p.fg_muted, bg = p.bg_alt })
  hl("TabLine", { fg = p.fg_muted, bg = "#3b3c35" })
  hl("TabLineFill", { bg = "#3b3c35" })
  hl("TabLineSel", { fg = p.fg, bg = p.bg, bold = true })
  hl("WinBar", { fg = p.fg, bg = p.bg_float })
  hl("WinBarNC", { fg = p.fg_muted, bg = p.bg_alt })
  hl("Pmenu", { fg = p.fg, bg = "#3b3c35" })
  hl("PmenuSel", { bg = "#e6db74", fg = "#272822" })
  hl("PmenuKind", { fg = p.orange, bg = p.bg_float })
  hl("PmenuKindSel", { fg = p.orange, bg = p.selection })
  hl("PmenuExtra", { fg = p.comment, bg = p.bg_float })
  hl("PmenuExtraSel", { fg = p.comment, bg = p.selection })
  hl("PmenuSbar", { bg = "#57584f" })
  hl("PmenuThumb", { bg = "#6e7066" })
  hl("PmenuMatch", { fg = p.blue, bg = p.bg_float, bold = true })
  hl("PmenuMatchSel", { fg = p.blue, bg = p.selection, bold = true })
  hl("ComplMatchIns", { fg = p.blue, bold = true })
  hl("Visual", { bg = "#3b3c35" })
  hl("VisualNOS", { bg = "#3b3c35" })
  hl("Search", { bg = "#e6db74", fg = "#272822" })
  hl("CurSearch", { bg = "#fd971f", bold = true, fg = "#272822" })
  hl("IncSearch", { bg = "#fd971f", fg = "#272822" })
  hl("Substitute", { fg = p.fg, bg = p.search_active, bold = true })
  hl("MatchParen", { fg = "#fdfff1", underline = false, bg = "#3b3c35", bold = true })
  hl("QuickFixLine", { bg = "#3b3c35", bold = true })
  hl("Folded", { fg = p.comment, bg = "#3b3c35" })
  hl("Directory", { fg = p.blue })
  hl("Title", { fg = p.blue, bold = true })
  hl("Question", { fg = "#66d9ef" })
  hl("MoreMsg", { fg = p.green })
  hl("ModeMsg", { fg = p.fg, bold = true })
  hl("MsgArea", { fg = p.fg, bg = p.bg })
  hl("MsgSeparator", { fg = p.border, bg = p.bg })
  hl("WarningMsg", { fg = "#fd971f", bold = true })
  hl("ErrorMsg", { fg = p.red, bold = true })
  hl("NonText", { fg = p.border })
  hl("EndOfBuffer", { fg = p.bg_float })
  hl("SpecialKey", { fg = p.border })
  hl("Whitespace", { fg = p.border })
  hl("Conceal", { fg = p.comment })
  hl("WildMenu", { bg = p.selection })
  hl("SnippetTabstop", { bg = p.selection })
  hl("SpellBad", { undercurl = true, sp = p.red })
  hl("SpellCap", { undercurl = true, sp = "#66d9ef" })
  hl("SpellRare", { undercurl = true, sp = "#ae81ff" })
  hl("SpellLocal", { undercurl = true, sp = "#fd971f" })
  hl("DiffAdd", { bg = "#1d1e19", fg = "#a6e22e" })
  hl("DiffChange", { bg = "#1d1e19", fg = "#fd971f" })
  hl("DiffDelete", { bg = "#1d1e19", fg = "#f92672" })
  hl("DiffText", { bg = "#1d1e19", bold = true, fg = "#fdfff1" })

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
  hl("@constructor", { link = "@type" })
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
  hl("DiagnosticWarn", { fg = "#fd971f" })
  hl("DiagnosticInfo", { fg = "#66d9ef" })
  hl("DiagnosticHint", { fg = "#ae81ff" })
  hl("DiagnosticOk", { fg = p.green })
  hl("DiagnosticVirtualTextError", { fg = p.red, italic = true })
  hl("DiagnosticVirtualTextWarn", { fg = "#fd971f", italic = true })
  hl("DiagnosticVirtualTextInfo", { fg = "#66d9ef", italic = true })
  hl("DiagnosticVirtualTextHint", { fg = "#ae81ff", italic = true })
  hl("DiagnosticVirtualTextOk", { fg = p.green, italic = true })
  hl("DiagnosticVirtualLinesError", { fg = p.red, italic = true })
  hl("DiagnosticVirtualLinesWarn", { fg = "#fd971f", italic = true })
  hl("DiagnosticVirtualLinesInfo", { fg = "#66d9ef", italic = true })
  hl("DiagnosticVirtualLinesHint", { fg = "#ae81ff", italic = true })
  hl("DiagnosticVirtualLinesOk", { fg = p.green, italic = true })
  hl("DiagnosticUnderlineError", { undercurl = true, sp = p.red })
  hl("DiagnosticUnderlineWarn", { undercurl = true, sp = "#fd971f" })
  hl("DiagnosticUnderlineInfo", { undercurl = true, sp = "#66d9ef" })
  hl("DiagnosticUnderlineHint", { undercurl = true, sp = "#ae81ff" })
  hl("DiagnosticUnderlineOk", { undercurl = true, sp = p.green })
  hl("DiagnosticFloatingError", { fg = "#f92672", bg = "#3b3c35" })
  hl("DiagnosticFloatingWarn", { fg = "#fd971f", bg = "#3b3c35" })
  hl("DiagnosticFloatingInfo", { fg = "#66d9ef", bg = "#3b3c35" })
  hl("DiagnosticFloatingHint", { fg = "#ae81ff", bg = "#3b3c35" })
  hl("DiagnosticFloatingOk", { fg = p.green })
  hl("DiagnosticSignError", { fg = p.red })
  hl("DiagnosticSignWarn", { fg = "#fd971f" })
  hl("DiagnosticSignInfo", { fg = "#66d9ef" })
  hl("DiagnosticSignHint", { fg = "#ae81ff" })
  hl("DiagnosticSignOk", { fg = p.green })
  hl("DiagnosticDeprecated", { strikethrough = true })
  hl("DiagnosticUnnecessary", { link = "Comment" })

  -- LSP
  hl("LspReferenceText", { bg = "#3b3c35" })
  hl("LspReferenceRead", { bg = "#3b3c35" })
  hl("LspReferenceWrite", { bg = "#3b3c35" })
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

  -- Override groups
  hl("GitSignsAdd", { fg = "#a6e22e" })
  hl("GitSignsChange", { fg = "#fd971f" })
  hl("GitSignsDelete", { fg = "#f92672" })
  hl("WhichKey", { fg = "#fd971f" })
  hl("WhichKeyGroup", { fg = "#66d9ef" })
  hl("WhichKeyDesc", { fg = "#fdfff1" })
  hl("WhichKeySeparator", { fg = "#6e7066" })
  hl("WhichKeyFloat", { bg = "#3b3c35" })
  hl("BlinkCmpMenu", { bg = "#3b3c35" })
  hl("BlinkCmpMenuBorder", { fg = "#6e7066", bg = "#3b3c35" })
  hl("BlinkCmpMenuSelection", { fg = "#272822", bg = "#e6db74" })
  hl("BlinkCmpLabelMatch", { fg = "#66d9ef", bold = true })
  hl("BlinkCmpKind", { fg = "#fd971f" })
  hl("SnacksPickerDir", { fg = "#6e7066" })
  hl("SnacksPickerFile", { fg = "#fdfff1" })
  hl("SnacksPickerMatch", { fg = "#66d9ef", bold = true })
  hl("SnacksPickerPrompt", { fg = "#fd971f" })
  hl("SnacksIndent", { fg = "#57584f" })
  hl("SnacksIndentScope", { fg = "#ae81ff" })
  hl("IblIndent", { fg = "#57584f" })
  hl("IblScope", { fg = "#ae81ff" })

  -- Plugin: GitSigns
  hl("GitSignsAdd", { fg = "#a6e22e" })
  hl("GitSignsChange", { fg = "#fd971f" })
  hl("GitSignsDelete", { fg = "#f92672" })

  -- Plugin: which-key
  hl("WhichKey", { fg = "#fd971f" })
  hl("WhichKeyGroup", { fg = "#66d9ef" })
  hl("WhichKeyDesc", { fg = "#fdfff1" })
  hl("WhichKeySeparator", { fg = "#6e7066" })
  hl("WhichKeyFloat", { bg = "#3b3c35" })

  -- Plugin: blink.cmp
  hl("BlinkCmpMenu", { fg = p.fg, bg = "#3b3c35" })
  hl("BlinkCmpMenuBorder", { fg = "#6e7066", bg = "#3b3c35" })
  hl("BlinkCmpMenuSelection", { bg = "#e6db74", fg = "#272822" })
  hl("BlinkCmpLabel", { fg = p.fg })
  hl("BlinkCmpLabelMatch", { fg = "#66d9ef", bold = true })
  hl("BlinkCmpKind", { fg = "#fd971f" })
  hl("BlinkCmpGhostText", { fg = p.ghost_text, italic = true })
  hl("MinuetVirtualText", { link = "BlinkCmpGhostText" })

  -- Plugin: snacks.nvim
  hl("SnacksPickerDir", { fg = "#6e7066" })
  hl("SnacksPickerFile", { fg = "#fdfff1" })
  hl("SnacksPickerPathIgnored", { fg = p.fg_inactive })
  hl("SnacksPickerPathHidden", { fg = p.fg_inactive })
  hl("SnacksPickerDimmed", { fg = p.fg_inactive })
  hl("SnacksPickerGitStatus", { fg = p.fg_inactive })
  hl("SnacksPickerGitStatusUntracked", { fg = p.fg_inactive })
  hl("SnacksPickerGitStatusIgnored", { fg = p.fg_inactive })
  hl("SnacksPickerMatch", { fg = "#66d9ef", bold = true })
  hl("SnacksPickerPrompt", { fg = "#fd971f" })
  hl("SnacksIndent", { fg = "#57584f" })
  hl("SnacksIndentScope", { fg = "#ae81ff" })

  -- Plugin: indent-blankline / ibl
  hl("IblIndent", { fg = "#57584f" })
  hl("IblScope", { fg = "#ae81ff" })
end

return M
