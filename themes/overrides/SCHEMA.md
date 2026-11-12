# Theme Overrides

Overrides are app-specific patches layered over canonical colors and app template defaults. Slot overrides apply before template rendering; other apps patch rendered fields as described below.

Use overrides when:

- the canonical `themes/core/*.yaml` roles are still correct overall
- an app has target-specific highlight semantics or UX expectations
- a specific theme needs a small amount of manual tuning for one app

Do not use overrides to redefine the canonical theme core.

## Current support

- `themes/overrides/ghostty/TEMPLATE.yaml`
- `themes/overrides/TEMPLATE.yaml` for Neovim
- `themes/overrides/starship/TEMPLATE.yaml`
- `themes/overrides/zed/TEMPLATE.yaml`
- `themes/overrides/zellij/TEMPLATE.yaml`
- `themes/overrides/ghostty/<theme-id>.yaml`
- `themes/overrides/nvim/<theme-id>.yaml`
- `themes/overrides/starship/<theme-id>.yaml`
- `themes/overrides/zed/<theme-id>.yaml`
- `themes/overrides/zellij/<theme-id>.yaml`
- `uv run --no-project themes/validate_overrides.py`
- `uv run --no-project themes/check_templates.py` also checks override template/current override file structure

Save each override as `themes/overrides/<app>/<theme-id>.yaml`. Use `version: 1`
and keep all top-level sections and `meta` keys in the order shown by that app's
template, even when a section is empty. Generation requires `meta.theme` and
`meta.variant` to match the canonical theme; standalone override validation
checks their format but does not compare them with the core file.

## Resolution order

For Ghostty exports, values resolve in this order:

1. generator-computed default slots
2. override `slots.<slot>` replacement

For Neovim exports, values resolve in this order:

1. rendered template defaults
2. override `groups.<group>` patch
3. override `links.<group>` replacement

For groups present in a template, a link override replaces the final attrs table. For a newly added group absent from both templates, a `groups` entry takes precedence over a same-named `links` entry.

For Starship exports, `slots` patches replace generator-computed slots before template rendering. `modules.<table>.<key>` patches replace existing string-valued template settings; other settings retain their template values. See the [Starship resolution rules](../README.md#resolution-rules) and `themes/overrides/starship/TEMPLATE.yaml` for supported values and authoring structure.

For Zed exports, `style` patches replace native style colors, `syntax.<capture>` patches merge highlight attributes, and `players.player_n` patches merge colors into an existing player entry. Unpatched attributes and players inherit template defaults. See [Zed resolution rules](../README.md#resolution-rules) and `themes/overrides/zed/TEMPLATE.yaml`. Style and player colors accept `#RRGGBB`, `#RRGGBBAA`, and canonical references. Syntax accepts the official `color`, `background_color`, `font_style`, and `font_weight` fields, including `null` to clear an inherited attribute.

For Zellij exports, values resolve in this order:

1. generator-computed component colors
2. override `components.<component>` attr patch
3. override `players.<player_n>` replacement inside `multiplayer_user_colors`
4. player-color deduplication and background-distinction adjustment

The final step can adjust even explicitly overridden player colors.

## Ghostty override format

Author from `themes/overrides/ghostty/TEMPLATE.yaml`:

```yaml
version: 1

meta:
  app: ghostty
  theme: rose-pine-moon
  variant: dark

slots:
  cursor_color: "#e0def4"
  palette_0_entry: "0=#393552"
```

## Neovim override format

Author from `themes/overrides/TEMPLATE.yaml`:

```yaml
version: 1

meta:
  app: nvim
  theme: rose-pine-moon
  variant: dark

groups:
  CurSearch:
    fg: "{roles.ui.bg}"
    bg: "{palette.magenta}"
  IncSearch:
    fg: "{roles.ui.bg}"
    bg: "{palette.magenta}"

links:
  @constructor: "@type"
```

## Zellij override format

Author from `themes/overrides/zellij/TEMPLATE.yaml`:

```yaml
version: 1

meta:
  app: zellij
  theme: monokai-pro-classic
  variant: dark

components:
  text_selected:
    background: "#4a4c44"
  list_selected:
    background: "#4a4c44"

players:
  player_5: "{palette.cyanBright}"
```

## Supported value forms

Ghostty override values may be:

- literal hex strings like `"#232136"` for UI color slots
- palette refs like `"{palette.selection}"` or role refs like `"{roles.ui.cursor}"` for UI color slots
- indexed palette entries like `"0=#393552"` or `"0={roles.ansi.black}"`; `palette_n_entry` values must include an index and `=`
- a quoted decimal from `"0"` to `"1"` for `faint_opacity`, such as `"0.85"`

Neovim override values may be:

- literal strings like `"#403d52"` or `"@type"`
- palette refs like `"{palette.magenta}"`
- role refs like `"{roles.syntax.function}"`
- scalar booleans written as `true` / `false`
- `null`

Zellij override values may be:

- literal hex strings like `"#4a4c44"`
- palette refs like `"{palette.cyanBright}"`
- role refs like `"{roles.ui.selection}"`

## Notes

- use plain YAML keys for highlight groups, including names like `@function` or `@constructor`; do not quote the key names in this repo's minimal YAML format
- `groups` patches merge into the existing generated attrs for a group.
- `links` replace the group attrs with `{ link = ... }`.
- groups absent from the builtin template are appended in an `Override groups` section before the plugin sections; plugin groups are also patched in their own sections.
- Ghostty `slots` replace individual generated slot values after the default terminal mapping is computed.
- Zellij `components` patches replace individual component attrs after generator defaults are resolved.
- Zellij `players` patches replace individual `player_n` colors before the final deduplication and background-distinction adjustment.
- templates remain the official/default contract; overrides are the narrow exception layer.
