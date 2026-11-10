# Theme Overrides

Overrides are app-specific patches applied after the canonical core and app template have already been resolved.

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

## Resolution order

For Ghostty exports, values resolve in this order:

1. generator-computed default slots
2. override `slots.<slot>` replacement

For Neovim exports, values resolve in this order:

1. rendered template defaults
2. override `groups.<group>` patch
3. override `links.<group>` replacement

If a link override exists for a group, it replaces the final attrs table for that group.

For Starship exports, `slots` patches replace generator-computed slots before template rendering. `modules.<table>.<key>` patches replace existing string-valued template settings; other settings retain their template values. See the [Starship resolution rules](../README.md#resolution-rules) and `themes/overrides/starship/TEMPLATE.yaml` for supported values and authoring structure.

For Zed exports, `style` patches replace native style colors, `syntax.<capture>` patches merge highlight attributes, and `players.player_n` patches merge colors into an existing player entry. Unpatched attributes and players inherit template defaults. See [Zed resolution rules](../README.md#resolution-rules) and `themes/overrides/zed/TEMPLATE.yaml`. Style and player colors accept `#RRGGBB`, `#RRGGBBAA`, and canonical references. Syntax accepts the official `color`, `background_color`, `font_style`, and `font_weight` fields, including `null` to clear an inherited attribute.

For Zellij exports, values resolve in this order:

1. generator-computed component colors
2. override `components.<component>` attr patch
3. override `players.<player_n>` replacement inside `multiplayer_user_colors`

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
- indexed palette entries like `"0=#393552"`
- palette refs like `"{palette.selection}"`
- role refs like `"{roles.ui.cursor}"`
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
- unknown groups are appended as an extra override section at the end of the generated Neovim output.
- Ghostty `slots` replace individual generated slot values after the default terminal mapping is computed.
- Zellij `components` patches replace individual component attrs after generator defaults are resolved.
- Zellij `players` patches replace individual `player_n` colors after the multiplayer palette is generated.
- templates remain the official/default contract; overrides are the narrow exception layer.
