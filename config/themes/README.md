# Canonical themes

`config/themes/core/` contains the canonical YAML theme definitions. `config/themes/generate.py` resolves them through app templates and optional overrides, then writes app-ready files under `config/themes/exports/`. Nix modules consume these exports through `config/lib/themes.nix`.

## Design Goals

- Base24-friendly palette as the foundation
- Additional semantic roles for better editor and UI export quality
- Keep the core schema small, stable, and app-agnostic
- Move application-specific logic into adapters and optional overrides
- Easy to read and edit by hand
- Generated exports that Nix modules can install directly
- Export to terminals, Zed, Neovim, and other supported apps

## Directory layout

```text
config/themes/
├── README.md                  # this document
├── TEMPLATE.yaml              # strict authoring template
├── core/                      # canonical theme definitions
├── templates/                 # app templates and vendored schemas
├── overrides/                 # Ghostty, Neovim, Starship, Zed, Zellij YAML patches
├── exports/                   # ghostty, herdr, k9s, nvim, pi, starship, tmux, zed, zellij
├── generate.py                # Python exporters
├── validate.py                # canonical schema validator and YAML parser
├── validate_overrides.py      # app override validation
└── check_templates.py         # template and override consistency checks
```

## Canonical Model

The canonical schema has 3 layers:

1. `meta`
   - metadata such as name, variant, author, description
2. `palette`
   - raw named colors
   - includes Base24-compatible slots
3. `roles`
   - semantic mapping used by exporters
   - separates UI, syntax, diagnostics, VCS, ANSI

Application-specific settings should not live in the canonical schema.
Those belong in exporters and optional override files outside `core/`.

## Example YAML shape

```yaml
version: 1

meta:
  id: tokyo-night-ish
  name: Tokyo Night-ish
  variant: dark        # dark | light
  family: tokyo-night
  author: pylv
  description: Base24-oriented canonical theme example

palette:
  # Base24-compatible base slots
  base00: "#1a1b26"
  base01: "#16161e"
  base02: "#2f3549"
  base03: "#444b6a"
  base04: "#787c99"
  base05: "#a9b1d6"
  base06: "#cbccd1"
  base07: "#d5d6db"
  base08: "#c0caf5"
  base09: "#a9b1d6"
  base0A: "#0db9d7"
  base0B: "#9ece6a"
  base0C: "#b4f9f8"
  base0D: "#2ac3de"
  base0E: "#bb9af7"
  base0F: "#f7768e"
  base10: "#ff9e64"
  base11: "#e0af68"
  base12: "#9ece6a"
  base13: "#b4f9f8"
  base14: "#2ac3de"
  base15: "#bb9af7"
  base16: "#c0caf5"
  base17: "#565f89"

  # Strict alias set. Keep names and order stable across all themes.
  bg: "{palette.base00}"
  bgElevated: "{palette.base01}"
  bgAlt: "{palette.base02}"
  border: "{palette.base03}"
  muted: "{palette.base04}"
  comment: "{palette.base16}"
  fgMuted: "{palette.base04}"
  fgInactive: "{palette.base06}"
  fg: "{palette.base05}"
  fgBright: "{palette.base06}"
  fgMax: "{palette.base07}"

  red: "{palette.base08}"
  orange: "{palette.base09}"
  yellow: "{palette.base0A}"
  green: "{palette.base0B}"
  cyan: "{palette.base0C}"
  blue: "{palette.base0D}"
  magenta: "{palette.base0E}"
  pink: "{palette.base0F}"

  yellowBright: "{palette.base10}"
  greenBright: "{palette.base11}"
  cyanBright: "{palette.base12}"
  blueBright: "{palette.base13}"
  magentaBright: "{palette.base14}"

  selection: "{palette.base15}"
  lineNumber: "{palette.base16}"
  whitespace: "{palette.base17}"
  blackBright: "{palette.base03}"
  white: "{palette.base07}"

roles:
  ui:
    bg: "{palette.bg}"
    bgAlt: "{palette.bgAlt}"
    bgElevated: "{palette.bgElevated}"
    fg: "{palette.fg}"
    fgInactive: "{palette.fgInactive}"
    fgMuted: "{palette.fgMuted}"
    border: "{palette.border}"
    borderActive: "{palette.blue}"
    selection: "{palette.selection}"
    selectionInactive: "{palette.bgAlt}"
    currentLine: "{palette.bgElevated}"
    currentLineNumber: "{palette.fgBright}"
    cursor: "{palette.fg}"
    caretText: "{palette.bg}"
    search: "{palette.yellowBright}"
    searchText: "{palette.bg}"
    match: "{palette.border}"
    panelBg: "{palette.bgElevated}"
    sidebarBg: "{palette.bgElevated}"
    statuslineBg: "{palette.bgElevated}"
    statuslineFg: "{palette.fg}"

  syntax:
    text: "{palette.fg}"
    comment: "{palette.comment}"
    string: "{palette.green}"
    stringEscape: "{palette.cyan}"
    number: "{palette.orange}"
    constant: "{palette.orange}"
    keyword: "{palette.magenta}"
    operator: "{palette.fg}"
    variable: "{palette.fg}"
    parameter: "{palette.fg}"
    property: "{palette.red}"
    field: "{palette.red}"
    function: "{palette.blue}"
    method: "{palette.blue}"
    type: "{palette.yellow}"
    class: "{palette.yellow}"
    interface: "{palette.yellow}"
    namespace: "{palette.blue}"
    builtin: "{palette.cyan}"
    tag: "{palette.red}"
    attribute: "{palette.yellow}"
    punctuation: "{palette.fg}"
    link: "{palette.blue}"

  diagnostics:
    error: "{palette.red}"
    warning: "{palette.yellow}"
    info: "{palette.blue}"
    hint: "{palette.cyan}"
    ok: "{palette.green}"

  vcs:
    added: "{palette.green}"
    modified: "{palette.yellow}"
    removed: "{palette.red}"
    renamed: "{palette.blue}"
    conflict: "{palette.orange}"

  ansi:
    black: "{palette.bg}"
    red: "{palette.red}"
    green: "{palette.green}"
    yellow: "{palette.yellow}"
    blue: "{palette.blue}"
    magenta: "{palette.magenta}"
    cyan: "{palette.cyan}"
    white: "{palette.fg}"
    brightBlack: "{palette.blackBright}"
    brightRed: "{palette.pink}"
    brightGreen: "{palette.greenBright}"
    brightYellow: "{palette.yellowBright}"
    brightBlue: "{palette.blueBright}"
    brightMagenta: "{palette.magentaBright}"
    brightCyan: "{palette.cyanBright}"
    brightWhite: "{palette.white}"
```

## Schema Rules

### 1. Required Top-Level Keys

- `version`
- `meta`
- `palette`
- `roles`

There is no app-specific top-level key in the canonical schema.

### 2. Required `meta` Keys

- `id`
- `name`
- `variant`

Optional:

- `family`
- `author`
- `description`

### 3. Required `palette` Keys

At minimum, require Base24-compatible slots:

- `base00` through `base17`

Use the strict alias set from `config/themes/TEMPLATE.yaml`.
Do not add extra alias names casually, and prefer keeping alias names and
ordering identical across all theme files.

### 4. Required `roles` Groups

- `ui`
- `syntax`
- `diagnostics`
- `vcs`
- `ansi`

Not every leaf must be required on day 1, but exporters should have shared
fallback logic.

## Role Naming Conventions

### General naming rules

- use short semantic names, not target-specific names
- use lower camelCase for role leaf names
- avoid app vocabulary like `activityBar`, `gutter`, `tabBar`, `titleBar`
- prefer meaning over rendering detail
- a role name should make sense across at least 2 target families

Good examples:

- `ui.bg`
- `ui.bgElevated`
- `ui.selection`
- `syntax.comment`
- `syntax.function`
- `diagnostics.error`
- `vcs.added`

Bad examples:

- `ui.activityBarBackground`
- `ui.editorGutterModified`
- `ui.tabInactiveForeground`
- `syntax.textmateKeywordControl`

### Role group intent

#### `ui`

Use for shared interface roles that many apps have in some form.

Recommended leaves:

- `bg`
- `bgAlt`
- `bgElevated`
- `fg`
- `fgInactive`
- `fgMuted`
- `border`
- `borderActive`
- `selection`
- `selectionInactive`
- `currentLine`
- `currentLineNumber`
- `cursor`
- `caretText`
- `search`
- `searchText`
- `match`
- `panelBg`
- `sidebarBg`
- `statuslineBg`
- `statuslineFg`

#### `syntax`

Use for language and markup meaning, not parser-specific scope systems.

Recommended leaves:

- `text`
- `comment`
- `string`
- `stringEscape`
- `number`
- `constant`
- `keyword`
- `operator`
- `variable`
- `parameter`
- `property`
- `field`
- `function`
- `method`
- `type`
- `class`
- `interface`
- `namespace`
- `builtin`
- `tag`
- `attribute`
- `punctuation`
- `link`

#### `diagnostics`

Use for editor and tooling feedback states.

Recommended leaves:

- `error`
- `warning`
- `info`
- `hint`
- `ok`

#### `vcs`

Use for version control or diff meaning.

Recommended leaves:

- `added`
- `modified`
- `removed`
- `renamed`
- `conflict`

#### `ansi`

Use for terminal palette export only.

Recommended leaves:

- `black`
- `red`
- `green`
- `yellow`
- `blue`
- `magenta`
- `cyan`
- `white`
- `brightBlack`
- `brightRed`
- `brightGreen`
- `brightYellow`
- `brightBlue`
- `brightMagenta`
- `brightCyan`
- `brightWhite`

## Shared Rules

### 1. Semantic first

When choosing a role, prefer semantic meaning over source palette ancestry.
For example:

- comments should map to `syntax.comment` even if they come from `base16`
- current-line highlight should map to `ui.currentLine` even if one app calls
  it `lineHighlight`

### 2. Promote only shared meaning

A role should be added to the core schema only if:

- it appears useful across multiple targets, or
- it captures a stable semantic concept, not just one app's option name

If it only helps one app, keep it in exporter logic or an override file.

### 3. Prefer derivation over duplication

If a role can be cleanly derived from another role or from the palette, do not
add a new core role yet.
Examples:

- `panelBg` can often derive from `bgElevated`
- `selectionInactive` can often derive from `bgAlt` or `selection`

### 4. Keep contrast semantics stable

Across both light and dark themes, the intended meaning should remain stable:

- `bg` is the primary surface
- `bgElevated` is the raised or floating surface
- `fg` is the primary readable foreground
- `fgInactive` is secondary readable foreground for inactive UI
- `fgMuted` is quiet foreground for low-emphasis UI
- `comment` is quieter than `text`
- `selection` must remain visibly distinct from `bg`

### 5. Do not encode typography or effects into role names

Use color roles for color meaning only.
Do not create names like:

- `commentItalic`
- `errorUnderline`
- `strongBold`

Style details such as italic, bold, underline, or opacity should be handled by
exporters or a future separate styling layer.

### 6. Light and dark themes use the same role contract

Do not rename roles based on variant.
Only the color assignments change.
For example, both light and dark themes should expose the same role names such as:

- `ui.bg`
- `ui.fg`
- `syntax.keyword`
- `diagnostics.warning`

### 7. Alias palette names are helpers, not the contract

Names like `red`, `blue`, `fg`, `bg`, or `comment` in `palette` are convenience
aliases.
The stable public contract for exporters is the `roles` section.

## Authoring Rules

When creating a new theme file:

1. start from `config/themes/TEMPLATE.yaml`
2. keep top-level key order exactly as shown in the template
3. keep alias names and role names exactly as shown in the template
4. prefer palette or alias references over raw hex in `roles`
5. only use raw hex in `roles` when a theme truly needs a value that should not
   become a shared alias
6. keep the same leaf ordering in each role group as the template
7. if a value is theme-specific but the role name is shared, change only the
   value, not the structure
8. if you think a new alias or role is needed, update `README.md` and
   `TEMPLATE.yaml` first

In short, `README.md` defines the rules and `TEMPLATE.yaml` defines the exact
authoring shape.

Validation helper:

```bash
uv run --no-project config/themes/validate.py
uv run --no-project config/themes/validate.py config/themes/core/one-half-light.yaml
```

Override validation helper:

```bash
uv run --no-project config/themes/validate_overrides.py
uv run --no-project config/themes/validate_overrides.py config/themes/overrides/nvim/rose-pine-moon.yaml
```

Generation helper:

```bash
uv run --no-project config/themes/generate.py
uv run --no-project config/themes/generate.py config/themes/core/one-half-light.yaml
```

The generator prints export paths relative to `config/`, such as `themes/exports/ghostty/one-half-light.conf`. It renders, validates, and serializes every app export for all requested themes before writing any files. A generation or validation error in any app or theme leaves existing exports unchanged and creates no export files. Filesystem errors during the subsequent write phase can still leave partially updated exports.

Generator regression tests:

```bash
uv run --no-project -m unittest discover -s config/themes -p 'test_*.py'
```

Template consistency helper:

```bash
uv run --no-project config/themes/check_templates.py
```

This now checks both:

- app template JSON files under `config/themes/templates/`
- override template/current override YAML files under `config/themes/overrides/`

Current generators:

- `ghostty` -> `config/themes/exports/ghostty/*.conf`
- `herdr` -> `config/themes/exports/herdr/*.toml`
- `k9s` -> `config/themes/exports/k9s/*.yaml`
- `zed` -> `config/themes/exports/zed/*.json`
- `nvim` -> `config/themes/exports/nvim/*.lua`
- `starship` -> `config/themes/exports/starship/*.toml`
- `zellij` -> `config/themes/exports/zellij/*.kdl`
- `tmux` -> `config/themes/exports/tmux/*.conf`
- `pi` -> `config/themes/exports/pi/*.json`

Adapter templates and schema helpers:

- `config/themes/templates/zed/schema-v0.2.0.json` -> vendored official Zed theme JSON
  Schema
- `config/themes/templates/zed/official-template.json` -> Zed official template split
  into `style_sections`, `syntax_sections`, and `players`
  - note: Zed's official schema strictly enumerates outer `style` keys, but
    `style.syntax` remains open-ended and is validated as
    `HighlightStyleContent` entries rather than a fixed syntax-key list
- `config/themes/templates/ghostty/official-template.json` -> Ghostty theme template
  derived from the config reference and existing local terminal config
- `config/themes/templates/herdr/official-template.json` -> Herdr inline theme template derived from the v0.9.5 configuration structs and palette definitions; all scalar custom color fields are mapped to canonical roles or derived colors
- `config/themes/templates/k9s/official-template.json` -> K9s skin template derived
  from the official skins format and the existing local module skin definitions
- `config/themes/templates/nvim/official-template.json` -> Neovim builtin highlight
  template derived from official help
- `config/themes/templates/nvim/plugins.json` -> Neovim plugin-specific highlight template
- `config/themes/templates/starship/official-template.json` -> Starship prompt template
  derived from the Starship config contract plus the existing local prompt
  layout
- `config/themes/templates/zellij/official-template.json` -> Zellij theme template
  derived from the official Zellij theme definition specification
- `config/themes/templates/tmux/official-template.json` -> tmux statusline template
  derived from tmux style syntax and the existing local statusline layout
- `config/themes/templates/pi/official-template.json` -> Pi TUI theme template derived
  from Pi's published theme schema and bundled theme documentation
- `config/themes/overrides/ghostty/*.yaml` -> optional Ghostty slot overrides applied
  after generator defaults
- `config/themes/overrides/starship/*.yaml` -> optional Starship palette/format slots and string-valued module settings applied after generator defaults
- `config/themes/overrides/zed/*.yaml` -> optional Zed style, syntax, and player patches applied before export schema validation
- `config/themes/overrides/zellij/*.yaml` -> optional Zellij-specific
  component/player overrides applied after generator defaults
- `config/themes/check_templates.py` -> consistency check for app template metadata,
  contract fields, section layout, duplicate entries, and declared key coverage

## Resolution Rules

Neovim and Zellij override loaders run their existing schema validators before returning an override. They require `meta.theme` and `meta.variant` to match the canonical theme. Unsupported highlight attributes, component names, and player keys fail generation before any export is written.

Ghostty palette overrides use indexed entries such as `palette_0_entry: "0={roles.ansi.black}"`. The generator preserves the index and resolves the color after `=` as a palette reference, role reference, or literal hex color. Other Ghostty slots retain their existing whole-value resolution.

Zed overrides patch native `style` colors, merge `syntax` highlight attributes, and merge indexed `players.player_n` colors. Colors accept six-digit hex, eight-digit hex with alpha, or canonical palette/role references. See `overrides/zed/TEMPLATE.yaml` for the authoring shape. The generator validates keys, values, and theme metadata before applying patches, then validates the complete export against the vendored Zed schema.

Vira Graphene's Neovim and Zed overrides follow the Graphene definition in the [official Vira Theme 2026.9.4 package](https://open-vsx.org/api/vira/vsc-vira-theme/2026.9.4). They preserve the original cyan keywords and punctuation, pink booleans, teal UI accents, and subdued line numbers where the apps expose corresponding highlights. Neovim uses slightly brighter comments, `#676767`, for readability; Zed retains the original comment color. Zed uses the original translucent gray selection, `#47474780`. Neovim's Visual highlight cannot blend its background, so it uses the brighter gray `#545454` to keep selections distinct from the editor background. Language-specific scope differences remain between VS Code, Neovim, and Zed.

Starship overrides use the ordered keys `version`, `meta`, `slots`, and `modules`; see `overrides/starship/TEMPLATE.yaml`. Color slots accept hex colors or canonical palette/role references. The `format` slot accepts a Starship format string. Module patches are limited to existing string-valued settings in the Starship template and inherit all other settings. The generator rejects invalid override keys and mismatched theme metadata. One Half Light separates the folder icon on a purple background from the directory name on a blue background with regular-weight bright text. The Git branch, ahead/behind arrows, and commit counts use blue text on gray, while the time uses dark text. Language and environment segments use accent-colored text. Git change indicators, the SSH hostname, and error/Vim mode indicators use canonical palette colors. These accents prioritize palette consistency and do not meet the 4.5:1 contrast target retained for ordinary text.

Exporters should resolve values in this order:

1. per-app override outside the core schema
2. `roles.*`
3. `palette.*`
4. exporter fallback defaults

This keeps app-specific tweaks possible without weakening the canonical schema.

## Core vs Adapter Boundary

### What belongs in the core schema

Only values that are:

- cross-application
- semantic rather than app-key specific
- likely to be reused by multiple exporters
- stable over time

Examples:

- `roles.ui.bg`
- `roles.ui.selection`
- `roles.syntax.comment`
- `roles.syntax.string`
- `roles.syntax.function`
- `roles.diagnostics.error`
- `roles.vcs.added`
- `roles.ansi.red`

### What should stay out of the core schema

App-specific keys and layout details that are really just target configuration.

Examples:

- VS Code `activityBarBadge.background`
- VS Code `editorGutter.modifiedBackground`
- Zed title bar or player-specific UI fragments
- any exact output key that only one target understands

Those belong in:

- exporter logic, when the mapping is generally derivable from roles
- per-app override files, when the target needs manual tuning

## Why This Structure

### Why Base24 as the palette base

- strong compatibility with terminal-oriented theming
- easy ANSI mapping
- existing ecosystem and mental model
- enough slots to avoid overloading the original Base16 meanings too hard

### Why not only Base24 slots

Base24 slots alone are not expressive enough for:

- VS Code workbench colors
- semantic token colors
- diagnostics and VCS states
- future non-terminal UI theming

That is why `roles` should be first-class.

### Why not keep app-specific config in the core

Because it tends to:

- pollute the canonical schema with transient target details
- make the source of truth harder to understand
- force app vocabulary into a supposedly shared model
- create pressure to add more one-off keys over time

A small stable core plus adapters is easier to maintain.

### Why YAML

- readable in reviews
- friendly for hand editing
- parsed by the repository's Python validator and generator
- easy to transform into JSON outputs

## Override Strategy

Default rule:

- do not write per-app settings into `config/themes/core/*.yaml`

Instead use this progression:

1. derive from `roles`
2. fallback to `palette`
3. if still not good enough, add a small override file in `config/themes/overrides/<app>/`

This keeps exceptions visible and isolated.

Current override support:

- `config/themes/overrides/SCHEMA.md` -> override-layer format notes
- `config/themes/overrides/ghostty/TEMPLATE.yaml` -> Ghostty minimal authoring template
- `config/themes/overrides/TEMPLATE.yaml` -> Neovim minimal authoring template
- `config/themes/overrides/starship/TEMPLATE.yaml` -> Starship minimal authoring template
- `config/themes/overrides/zed/TEMPLATE.yaml` -> Zed minimal authoring template
- `config/themes/overrides/zellij/TEMPLATE.yaml` -> Zellij minimal authoring template
- `config/themes/overrides/ghostty/<theme-id>.yaml` -> Ghostty slot overrides
- `config/themes/overrides/nvim/<theme-id>.yaml` -> Neovim per-theme override patches
- `config/themes/overrides/starship/<theme-id>.yaml` -> Starship slot and module setting overrides
- `config/themes/overrides/zed/<theme-id>.yaml` -> Zed style, syntax, and player overrides
- `config/themes/overrides/zellij/<theme-id>.yaml` -> Zellij per-theme override patches
- `config/themes/validate_overrides.py` -> override validator for Ghostty, Neovim, Starship, Zed, and Zellij override files

Current Neovim override precedence:

1. rendered template attrs
2. `groups.<highlight-group>` patch merge
3. `links.<highlight-group>` replacement

Use overrides for app-specific exceptions like:

- matching an official app theme more closely for one theme only
- correcting interaction colors such as search, selection, or float emphasis
- nudging exact highlight groups without changing canonical roles

## Nix integration

`config/lib/themes.nix` exposes the generated files to Home Manager through the `themeExports` argument. `dir` and `file` return immutable flake-source paths. `mutableDir` and `mutableFile` return checkout paths; `mutableDirLink` and `mutableFileLink` wrap those paths in out-of-store symlinks.

`config/base/default.nix` defines `modules.commonTheme`, which theme-aware modules use to select an export. App installation behavior belongs to each module. Theme loading, validation, color derivation, and export generation remain in the Python pipeline.

The default `one-half-light` theme uses a cool white background (`#fafbfc`) and cool gray panels and borders (`#f3f5f7`, `#edf0f3`, `#e2e6ea`) to avoid a yellow cast. Ghostty inherits the canonical background. Syntax, diagnostic, and search accents retain their semantic colors.

Ghostty exports include `faint-opacity`, which defaults to `0.5`. A Ghostty override can set `slots.faint_opacity` to a quoted decimal from `0` to `1`; One Half Light uses `0.85` to keep faint secondary text readable. The validator rejects values outside this range. ANSI colors remain unchanged.

Herdr exports contain `[theme]` and `[theme.custom]` tables. The module combines the selected export with its base configuration into a read-only Home Manager file. The current space and agent share an active-row background derived by mixing `roles.ui.sidebarBg` toward `roles.ui.fg` in 0.1% steps until it reaches 1.8:1 contrast against the sidebar while retaining at least 4.5:1 contrast with normal text. This produces a darker gray in One Half Light and remains separate from the keyboard navigation selection color. Running green starts from `palette.green` and moves toward `roles.ui.fg` as needed to reach 1.8:1 sRGB contrast against sidebar and active-row backgrounds. This leaves margin above Ghostty's configured 1.6 minimum; regression tests also check the Display P3 conversion used by its macOS Metal renderer. The literal Claude/Codex subagent running colors in [Herdr's configuration](../modules/herdr/files/config.toml) match the One Half Light export and are checked against every theme's sidebar and active-row backgrounds. Secondary text starts from `roles.ui.fgInactive` and moves toward `roles.ui.fg` only as needed to reach 4.5:1 contrast against the main, panel, sidebar, alternate, and active-row backgrounds. The `mauve` color used for focused space details follows the same rule, starting from `palette.magenta`. Herdr draws selected tab labels with the panel color on its accent color, so the exported accent is similarly adjusted from `roles.ui.borderActive` to reach 4.5:1 against the panel. Generation fails if the active-row background or these foregrounds cannot meet their targets. Subagent activity rows use the secondary color without an additional dim modifier. Other accent colors retain their canonical mappings and are not covered by these contrast targets.

The current generator emits Ghostty, Herdr, K9s, Neovim, Pi, Starship, tmux, Zed, and Zellij themes. VSCode's app-local theme is outside this pipeline. Edit canonical definitions, templates, or supported YAML overrides, then regenerate the exports instead of editing generated files directly.
