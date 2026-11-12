# Canonical themes

`themes/core/` contains the canonical YAML theme definitions. `themes/generate.py` resolves them through app templates and optional overrides, then writes app-ready files under `themes/exports/`. Nix modules consume these exports through `lib/themes.nix`.

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
themes/
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

Copy [`TEMPLATE.yaml`](TEMPLATE.yaml) for the complete authoring shape, or see
[`core/one-half-light.yaml`](core/one-half-light.yaml) for a current light theme.
The validator requires the template's exact keys and ordering, not a minimal
subset.

## Schema Rules

### 1. Required Top-Level Keys

- `version`
- `meta`
- `palette`
- `roles`

There is no app-specific top-level key in the canonical schema.

### 2. Required `meta` Keys

- `id` (kebab-case)
- `name`
- `variant` (`dark` or `light`)
- `family`
- `author`
- `description`

All six fields are required, in template order.

### 3. Required `palette` Keys

Require all 24 Base24-compatible slots, `base00` through `base17`, including
`base0A` through `base0F`, followed by the exact alias set from
`themes/TEMPLATE.yaml`. Missing keys, extra keys, and reordered keys fail
validation.

Palette and role colors accept `#RRGGBB` or `{palette.name}` references.
References to `roles.*` are supported in app overrides, not in canonical colors.

### 4. Required `roles` Groups

- `ui`
- `syntax`
- `diagnostics`
- `vcs`
- `ansi`

Every leaf in each template role group is required, in template order.
Exporters derive app-specific colors from this complete context; omitted
canonical roles do not fall back to palette colors or exporter defaults.

Duplicate mapping keys are rejected in both canonical themes and overrides, including repeated scalar, mapping, or block-text entries. An ambiguous input fails before generation replaces any exports.

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

| Group | Meaning |
| --- | --- |
| `ui` | Shared interface surfaces, foregrounds, selection, cursor, and search colors. |
| `syntax` | Language and markup meaning, independent of parser scope names. |
| `diagnostics` | Error, warning, info, hint, and success states. |
| `vcs` | Added, modified, removed, renamed, and conflicting content. |
| `ansi` | The 16 terminal palette colors. |

The exact required leaves are maintained in `themes/TEMPLATE.yaml`.

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

1. start from `themes/TEMPLATE.yaml`
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
uv run --no-project themes/validate.py
uv run --no-project themes/validate.py themes/core/one-half-light.yaml
```

Override validation helper:

```bash
uv run --no-project themes/validate_overrides.py
uv run --no-project themes/validate_overrides.py themes/overrides/nvim/rose-pine-moon.yaml
```

Generation helper:

```bash
uv run --no-project themes/generate.py
uv run --no-project themes/generate.py themes/core/one-half-light.yaml
```

The generator prints export paths relative to the repository root, such as `themes/exports/ghostty/one-half-light.conf`. It validates the canonical themes and supported overrides, checks Zed exports against the vendored schema, and renders and serializes all requested exports before writing any files. It does not run each application's native config validator. A generation or validation error in any app or theme leaves existing exports unchanged and creates no export files. Filesystem errors during the subsequent write phase can still leave partially updated exports.

Generator regression tests:

```bash
uv run --no-project -m unittest discover -s themes -p 'test_*.py'
```

Template consistency helper:

```bash
uv run --no-project themes/check_templates.py
```

This checks both:

- app template JSON files under `themes/templates/`
- override template/current override YAML files under `themes/overrides/`

Current generators:

- `ghostty` -> `themes/exports/ghostty/*.conf`
- `herdr` -> `themes/exports/herdr/*.toml`
- `k9s` -> `themes/exports/k9s/*.yaml`
- `zed` -> `themes/exports/zed/*.json`
- `nvim` -> `themes/exports/nvim/*.lua`
- `starship` -> `themes/exports/starship/*.toml`
- `zellij` -> `themes/exports/zellij/*.kdl`
- `tmux` -> `themes/exports/tmux/*.conf`
- `pi` -> `themes/exports/pi/*.json`

Adapter templates and schema helpers:

- `themes/templates/zed/schema-v0.2.0.json` -> vendored official Zed theme JSON
  Schema
- `themes/templates/zed/official-template.json` -> Zed official template split
  into `style_sections`, `syntax_sections`, and `players`
  - note: Zed's official schema strictly enumerates outer `style` keys, but
    `style.syntax` remains open-ended and is validated as
    `HighlightStyleContent` entries rather than a fixed syntax-key list
- `themes/templates/ghostty/official-template.json` -> Ghostty theme template
  derived from the config reference and existing local terminal config
- `themes/templates/herdr/official-template.json` -> Herdr inline theme template derived from the v0.9.5 configuration structs and palette definitions; all scalar custom color fields are mapped to canonical roles or derived colors
- `themes/templates/k9s/official-template.json` -> K9s skin template derived
  from the official skins format and the existing local module skin definitions
- `themes/templates/nvim/official-template.json` -> Neovim builtin highlight
  template derived from official help
- `themes/templates/nvim/plugins.json` -> Neovim plugin-specific highlight template
- `themes/templates/starship/official-template.json` -> Starship prompt template
  derived from the Starship config contract plus the existing local prompt
  layout
- `themes/templates/zellij/official-template.json` -> Zellij theme template
  derived from the official Zellij theme definition specification
- `themes/templates/tmux/official-template.json` -> tmux statusline template
  derived from tmux style syntax and the existing local statusline layout
- `themes/templates/pi/official-template.json` -> Pi TUI theme template derived
  from Pi's published theme schema and bundled theme documentation
- [`overrides/SCHEMA.md`](overrides/SCHEMA.md) -> supported override formats,
  authoring templates, and per-app merge rules
- `themes/check_templates.py` -> consistency check for app template metadata,
  contract fields, section layout, duplicate entries, and declared key coverage

## Resolution Rules

All supported override loaders validate their app's override structure. Generation also requires `meta.theme` and `meta.variant` to match the canonical theme. Unsupported highlight attributes, component names, and player keys fail generation before any export is written.

Ghostty palette overrides use indexed entries such as `palette_0_entry: "0={roles.ansi.black}"`. The generator preserves the index and resolves the color after `=` as a palette reference, role reference, or literal hex color. Other Ghostty slots retain their existing whole-value resolution.

Zed overrides patch native `style` colors, merge `syntax` highlight attributes, and merge indexed `players.player_n` colors. Colors accept six-digit hex, eight-digit hex with alpha, or canonical palette/role references. See `overrides/zed/TEMPLATE.yaml` for the authoring shape. The generator validates keys, values, and theme metadata before applying patches, then validates the complete export against the vendored Zed schema.

Vira Graphene's Neovim and Zed overrides follow the Graphene definition in the [official Vira Theme 2026.9.4 package](https://open-vsx.org/api/vira/vsc-vira-theme/2026.9.4). They preserve the original cyan keywords and punctuation, pink booleans, teal UI accents, and subdued line numbers where the apps expose corresponding highlights. Neovim uses slightly brighter comments, `#676767`, for readability; Zed retains the original comment color. Zed uses the original translucent gray selection, `#47474780`. Neovim's Visual highlight cannot blend its background, so it uses the brighter gray `#545454` to keep selections distinct from the editor background. Language-specific scope differences remain between VS Code, Neovim, and Zed.

Starship overrides use the ordered keys `version`, `meta`, `slots`, and `modules`; see `overrides/starship/TEMPLATE.yaml`. Color slots accept hex colors or canonical palette/role references. The `format` slot accepts a Starship format string. Module patches are limited to existing string-valued settings in the Starship template and inherit all other settings. The generator rejects invalid override keys and mismatched theme metadata. One Half Light separates the folder icon on a purple background from the directory name on a blue background with regular-weight bright text. The Git branch, ahead/behind arrows, and commit counts use blue text on gray, while the time uses dark text. Language and environment segments use accent-colored text. Git change indicators, the SSH hostname, and error/Vim mode indicators use canonical palette colors. These accents prioritize palette consistency and do not meet the 4.5:1 contrast target retained for ordinary text.

Resolution proceeds from canonical data to app output:

1. validate the complete canonical theme and resolve palette references
2. resolve role colors from that palette
3. compute app-specific slots and render templates, applying supported overrides
   at the stage described in [`overrides/SCHEMA.md`](overrides/SCHEMA.md)
4. validate the Zed export and serialize the app outputs

Overrides can replace derived app values; they do not make canonical keys optional.

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
- do not write per-app settings into `themes/core/*.yaml`

Instead use this progression:

1. derive from `roles`
2. if still not good enough, add a small override file in `themes/overrides/<app>/`

This keeps exceptions visible and isolated.

Ghostty, Neovim, Starship, Zed, and Zellij support overrides. See
[`overrides/SCHEMA.md`](overrides/SCHEMA.md) for the templates, supported values,
and merge precedence. Herdr, K9s, Pi, and tmux currently use their canonical
theme mappings and app templates without a YAML override layer.

Use overrides for app-specific exceptions like:

- matching an official app theme more closely for one theme only
- correcting interaction colors such as search, selection, or float emphasis
- nudging exact highlight groups without changing canonical roles

## Nix integration
`lib/themes.nix` exposes the generated files to Home Manager through the `themeExports` argument. `dir` and `file` return immutable flake-source paths. `mutableDir` and `mutableFile` return checkout paths; `mutableDirLink` and `mutableFileLink` wrap those paths in out-of-store symlinks.
`systems/base/default.nix` defines `modules.commonTheme`, which theme-aware modules use to select an export. App installation behavior belongs to each module. Theme loading, validation, color derivation, and export generation remain in the Python pipeline.

The default `one-half-light` theme uses a cool white background (`#fafbfc`) and cool gray panels and borders (`#f3f5f7`, `#edf0f3`, `#e2e6ea`) to avoid a yellow cast. Ghostty inherits the canonical background. Syntax, diagnostic, and search accents retain their semantic colors.

Ghostty exports include `faint-opacity`, which defaults to `0.5`. A Ghostty override can set `slots.faint_opacity` to a quoted decimal from `0` to `1`; One Half Light uses `0.85` to keep faint secondary text readable. The validator rejects values outside this range. ANSI colors remain unchanged.

Herdr exports contain `[theme]` and `[theme.custom]` tables. The module combines the selected export with its base configuration into a read-only Home Manager file. The current space and agent share an active-row background derived by mixing `roles.ui.sidebarBg` toward `roles.ui.fg` in 0.1% steps until it reaches 1.8:1 contrast against the sidebar while retaining at least 4.5:1 contrast with normal text. This produces a darker gray in One Half Light and remains separate from the keyboard navigation selection color. Running green starts from `palette.green` and moves toward `roles.ui.fg` as needed to reach 1.8:1 sRGB contrast against sidebar and active-row backgrounds. This leaves margin above Ghostty's configured 1.6 minimum; regression tests also check the Display P3 conversion used by its macOS Metal renderer. The literal Claude/Codex subagent running colors in [Herdr's configuration](../modules/herdr/files/config.toml) match the One Half Light export and are checked against every theme's sidebar and active-row backgrounds. Secondary text starts from `roles.ui.fgInactive` and moves toward `roles.ui.fg` only as needed to reach 4.5:1 contrast against the main, panel, sidebar, alternate, and active-row backgrounds. The `mauve` color used for focused space details follows the same rule, starting from `palette.magenta`. Herdr draws selected tab labels with the panel color on its accent color, so the exported accent is similarly adjusted from `roles.ui.borderActive` to reach 4.5:1 against the panel. Generation fails if the active-row background or these foregrounds cannot meet their targets. Subagent activity rows use the secondary color without an additional dim modifier. Other accent colors retain their canonical mappings and are not covered by these contrast targets.

Pi tool result backgrounds use subdued palette blue for success and palette orange for errors. General success and error indicators retain their diagnostic colors, and tool diffs retain their VCS colors.

The current generator emits Ghostty, Herdr, K9s, Neovim, Pi, Starship, tmux, Zed, and Zellij themes. VSCode's app-local theme is outside this pipeline. Edit canonical definitions, templates, or supported YAML overrides, then regenerate the exports instead of editing generated files directly.
