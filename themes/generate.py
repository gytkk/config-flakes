#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

from validate import ParseError, Validator, load_yaml, validate_theme


REF_RE = re.compile(r"^\{palette\.([A-Za-z][A-Za-z0-9]*)\}$")
ROLE_REF_RE = re.compile(r"^\{roles\.([A-Za-z][A-Za-z0-9]*)\.([A-Za-z][A-Za-z0-9]*)\}$")


def alpha(hex_color: str, suffix: str) -> str:
    return f"{hex_color}{suffix}"


def hex_to_rgb(hex_color: str) -> tuple[int, int, int]:
    value = hex_color.removeprefix("#")
    return tuple(int(value[i : i + 2], 16) for i in (0, 2, 4))  # type: ignore[return-value]


def rgb_to_hex(rgb: tuple[int, int, int]) -> str:
    return "#%02x%02x%02x" % rgb


def mix(hex_a: str, hex_b: str, ratio_b: float) -> str:
    ratio_a = 1.0 - ratio_b
    a = hex_to_rgb(hex_a)
    b = hex_to_rgb(hex_b)
    mixed = tuple(round(a[i] * ratio_a + b[i] * ratio_b) for i in range(3))
    return rgb_to_hex(mixed)


def soft_background(accent: str, bg: str, *, light: bool, strength: float = 0.82) -> str:
    ratio_bg = strength if light else min(0.88, strength + 0.04)
    return mix(accent, bg, ratio_bg)


def raised_surface(bg_alt: str, border: str, *, light: bool) -> str:
    return mix(bg_alt, border, 0.22 if light else 0.14)


def sharpen(accent: str, bg: str, *, light: bool) -> str:
    return mix(accent, bg, 0.58 if light else 0.28)


def dim_terminal(color: str, bg: str, *, light: bool) -> str:
    return mix(color, bg, 0.34 if light else 0.42)


def color_distance(hex_a: str, hex_b: str) -> int:
    a = hex_to_rgb(hex_a)
    b = hex_to_rgb(hex_b)
    return sum(abs(a[i] - b[i]) for i in range(3))


def relative_luminance(hex_color: str) -> float:
    def channel(value: int) -> float:
        c = value / 255.0
        if c <= 0.03928:
            return c / 12.92
        return ((c + 0.055) / 1.055) ** 2.4

    r, g, b = hex_to_rgb(hex_color)
    rl, gl, bl = channel(r), channel(g), channel(b)
    return 0.2126 * rl + 0.7152 * gl + 0.0722 * bl


def contrast_ratio(hex_a: str, hex_b: str) -> float:
    a = relative_luminance(hex_a)
    b = relative_luminance(hex_b)
    lighter = max(a, b)
    darker = min(a, b)
    return (lighter + 0.05) / (darker + 0.05)


def best_contrast(bg: str, *candidates: str) -> str:
    return max(candidates, key=lambda candidate: contrast_ratio(candidate, bg))


def ensure_distinct_background(candidate: str, base: str, accent: str, *, light: bool) -> str:
    if color_distance(candidate, base) >= 72:
        return candidate
    fallback = soft_background(accent, base, light=light, strength=0.58 if light else 0.52)
    if color_distance(fallback, base) >= 72:
        return fallback
    return mix(accent, base, 0.46 if light else 0.34)


def load_and_validate_theme(theme_path: Path, template: dict[str, Any]) -> dict[str, Any]:
    validator = Validator()
    validate_theme(validator, theme_path, template)
    if validator.errors:
        raise RuntimeError("\n".join(validator.errors))
    return load_yaml(theme_path)


def resolve_palette(theme: dict[str, Any]) -> dict[str, str]:
    palette = theme["palette"]
    resolved: dict[str, str] = {}
    resolving: set[str] = set()

    def resolve(name: str) -> str:
        if name in resolved:
            return resolved[name]
        if name in resolving:
            raise RuntimeError(f"cyclic palette reference: {name}")
        if name not in palette:
            raise RuntimeError(f"missing palette key: {name}")
        resolving.add(name)
        value = palette[name]
        if not isinstance(value, str):
            raise RuntimeError(f"palette.{name} must be a string")
        match = REF_RE.match(value)
        if match:
            final = resolve(match.group(1))
        else:
            final = value
        resolving.remove(name)
        resolved[name] = final
        return final

    for key in palette:
        resolve(key)
    return resolved


def resolve_value(value: str, resolved_palette: dict[str, str]) -> str:
    match = REF_RE.match(value)
    if match:
        return resolved_palette[match.group(1)]
    return value


def resolve_roles(theme: dict[str, Any], resolved_palette: dict[str, str]) -> dict[str, dict[str, str]]:
    out: dict[str, dict[str, str]] = {}
    for group, values in theme["roles"].items():
        out[group] = {key: resolve_value(val, resolved_palette) for key, val in values.items()}
    return out


def theme_context(theme: dict[str, Any]) -> dict[str, Any]:
    palette = resolve_palette(theme)
    roles = resolve_roles(theme, palette)
    return {
        "meta": theme["meta"],
        "palette": palette,
        "roles": roles,
    }


def resolve_context_value(value: Any, ctx: dict[str, Any]) -> Any:
    if isinstance(value, str):
        palette_match = REF_RE.match(value)
        if palette_match:
            return ctx["palette"][palette_match.group(1)]

        role_match = ROLE_REF_RE.match(value)
        if role_match:
            group, key = role_match.groups()
            return ctx["roles"][group][key]

        if value == "true":
            return True
        if value == "false":
            return False
        if value == "null":
            return None
        return value

    if isinstance(value, dict):
        return {key: resolve_context_value(item, ctx) for key, item in value.items()}
    if isinstance(value, list):
        return [resolve_context_value(item, ctx) for item in value]
    return value


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def load_zed_schema(root: Path) -> dict[str, Any]:
    return load_json(root / "templates" / "zed" / "schema-v0.2.0.json")


def load_zed_template(root: Path) -> dict[str, Any]:
    return load_json(root / "templates" / "zed" / "official-template.json")


def _schema_check_type(expected: Any, value: Any) -> bool:
    if isinstance(expected, list):
        return any(_schema_check_type(item, value) for item in expected)
    if expected == "object":
        return isinstance(value, dict)
    if expected == "array":
        return isinstance(value, list)
    if expected == "string":
        return isinstance(value, str)
    if expected == "integer":
        return isinstance(value, int) and not isinstance(value, bool)
    if expected == "number":
        return (isinstance(value, int) or isinstance(value, float)) and not isinstance(value, bool)
    if expected == "boolean":
        return isinstance(value, bool)
    if expected == "null":
        return value is None
    return True


def _resolve_schema_ref(ref: str, schema: dict[str, Any]) -> dict[str, Any]:
    if not ref.startswith("#/definitions/"):
        raise RuntimeError(f"unsupported schema ref: {ref}")
    name = ref.removeprefix("#/definitions/")
    return schema["definitions"][name]


def _validate_against_schema(node: dict[str, Any], value: Any, schema: dict[str, Any], path: str, errors: list[str]) -> None:
    if "$ref" in node:
        _validate_against_schema(_resolve_schema_ref(node["$ref"], schema), value, schema, path, errors)
        return

    if "anyOf" in node:
        branch_errors: list[list[str]] = []
        for option in node["anyOf"]:
            local_errors: list[str] = []
            _validate_against_schema(option, value, schema, path, local_errors)
            if not local_errors:
                break
            branch_errors.append(local_errors)
        else:
            errors.append(f"{path}: value did not match any allowed schema branch")
        return

    if "enum" in node and value not in node["enum"]:
        errors.append(f"{path}: expected one of {node['enum']!r}, got {value!r}")
        return

    expected_type = node.get("type")
    if expected_type is not None and not _schema_check_type(expected_type, value):
        errors.append(f"{path}: expected type {expected_type!r}, got {type(value).__name__}")
        return

    if isinstance(value, dict):
        required = node.get("required", [])
        for key in required:
            if key not in value:
                errors.append(f"{path}: missing required key {key!r}")

        properties = node.get("properties", {})
        additional = node.get("additionalProperties", None)
        for key, item in value.items():
            child_path = f"{path}.{key}" if path else key
            if key in properties:
                _validate_against_schema(properties[key], item, schema, child_path, errors)
            elif isinstance(additional, dict):
                _validate_against_schema(additional, item, schema, child_path, errors)

    if isinstance(value, list) and "items" in node:
        for idx, item in enumerate(value):
            _validate_against_schema(node["items"], item, schema, f"{path}[{idx}]", errors)


def validate_zed_theme_doc(doc: dict[str, Any], root: Path) -> None:
    schema = load_zed_schema(root)
    template = load_zed_template(root)
    errors: list[str] = []
    _validate_against_schema(schema, doc, schema, "theme", errors)

    allowed_style_keys = set(template["style_keys"])
    for idx, theme in enumerate(doc.get("themes", [])):
        style = theme.get("style", {})
        unknown = sorted(key for key in style.keys() if key not in allowed_style_keys and key not in {"players", "syntax"})
        if unknown:
            errors.append(f"theme.themes[{idx}].style: unknown style keys: {', '.join(unknown)}")

    if errors:
        raise RuntimeError("Zed theme schema validation failed:\n" + "\n".join(errors))


def render_template_value(value: Any, slots: dict[str, Any]) -> Any:
    if isinstance(value, str) and value.startswith("$"):
        name = value[1:]
        if name not in slots:
            raise RuntimeError(f"missing template slot: {name}")
        return slots[name]
    if isinstance(value, dict):
        return {key: render_template_value(item, slots) for key, item in value.items()}
    if isinstance(value, list):
        return [render_template_value(item, slots) for item in value]
    return value


def build_zed_slots(ctx: dict[str, Any]) -> dict[str, Any]:
    meta = ctx["meta"]
    p = ctx["palette"]
    r = ctx["roles"]
    is_light = meta["variant"] == "light"

    players = [p["blue"], p["orange"], p["yellow"], p["cyan"], p["green"], p["red"], p["blueBright"], p["magentaBright"]]
    slots: dict[str, Any] = {
        "border": alpha(r["ui"]["border"], "ff"),
        "border_variant": alpha(p["whitespace"], "ff"),
        "border_focused": alpha(r["ui"]["borderActive"], "ff"),
        "border_selected": alpha(r["ui"]["selection"], "ff"),
        "border_transparent": "#00000000",
        "border_disabled": alpha(r["ui"]["border"], "ff"),
        "elevated_surface_background": alpha(r["ui"]["bgElevated"], "ff"),
        "surface_background": alpha(r["ui"]["panelBg"], "ff"),
        "background": alpha(r["ui"]["bgAlt"], "ff"),
        "element_background": alpha(r["ui"]["bgElevated"], "ff"),
        "element_hover": alpha(r["ui"]["currentLine"], "ff"),
        "element_active": alpha(r["ui"]["selection"], "ff"),
        "element_selected": alpha(r["ui"]["selection"], "ff"),
        "element_disabled": alpha(r["ui"]["bgElevated"], "ff"),
        "drop_target_background": alpha(r["ui"]["selection"], "80"),
        "ghost_element_background": "#00000000",
        "ghost_element_hover": alpha(r["ui"]["currentLine"], "ff"),
        "ghost_element_active": alpha(r["ui"]["selection"], "ff"),
        "ghost_element_selected": alpha(r["ui"]["selection"], "ff"),
        "ghost_element_disabled": alpha(r["ui"]["bgElevated"], "ff"),
        "text": alpha(r["ui"]["fg"], "ff"),
        "text_muted": alpha(r["ui"]["fgInactive"], "ff"),
        "text_placeholder": alpha(p["lineNumber"], "ff"),
        "text_disabled": alpha(p["lineNumber"], "ff"),
        "text_accent": alpha(r["syntax"]["function"], "ff"),
        "icon": alpha(r["ui"]["fg"], "ff"),
        "icon_muted": alpha(r["ui"]["fgInactive"], "ff"),
        "icon_disabled": alpha(p["lineNumber"], "ff"),
        "icon_placeholder": alpha(r["ui"]["fgMuted"], "ff"),
        "icon_accent": alpha(r["syntax"]["function"], "ff"),
        "link_text_hover": alpha(r["syntax"]["link"], "ff"),
        "status_bar_background": alpha(r["ui"]["statuslineBg"], "ff"),
        "title_bar_background": alpha(r["ui"]["panelBg"], "ff"),
        "title_bar_inactive_background": alpha(r["ui"]["panelBg"], "ff"),
        "toolbar_background": alpha(r["ui"]["bg"], "ff"),
        "tab_bar_background": alpha(r["ui"]["bgAlt"], "ff"),
        "tab_inactive_background": alpha(r["ui"]["bgAlt"], "ff"),
        "tab_active_background": alpha(r["ui"]["bg"], "ff"),
        "search_match_background": alpha(r["ui"]["search"], "80"),
        "panel_background": alpha(r["ui"]["panelBg"], "ff"),
        "panel_focused_border": alpha(r["ui"]["borderActive"], "ff"),
        "panel_indent_guide": alpha(r["ui"]["border"], "66"),
        "panel_indent_guide_active": alpha(r["ui"]["borderActive"], "99"),
        "panel_indent_guide_hover": alpha(r["ui"]["fgMuted"], "80"),
        "pane_focused_border": alpha(r["ui"]["borderActive"], "ff"),
        "pane_group_border": alpha(r["ui"]["border"], "ff"),
        "scrollbar_thumb_background": alpha(r["ui"]["fgMuted"], "40"),
        "scrollbar_thumb_hover_background": alpha(r["ui"]["fgMuted"], "60"),
        "scrollbar_thumb_border": alpha(r["ui"]["border"], "ff"),
        "scrollbar_track_background": "#00000000",
        "scrollbar_track_border": alpha(r["ui"]["border"], "ff"),
        "editor_foreground": alpha(r["ui"]["fg"], "ff"),
        "editor_background": alpha(r["ui"]["bg"], "ff"),
        "editor_gutter_background": alpha(r["ui"]["bg"], "ff"),
        "editor_subheader_background": alpha(r["ui"]["bgElevated"], "ff"),
        "editor_active_line_background": alpha(r["ui"]["currentLine"], "bf"),
        "editor_highlighted_line_background": alpha(r["ui"]["currentLine"], "ff"),
        "editor_line_number": alpha(p["lineNumber"], "ff"),
        "editor_active_line_number": alpha(r["ui"]["currentLineNumber"], "ff"),
        "editor_invisible": alpha(p["whitespace"], "ff"),
        "editor_wrap_guide": alpha(r["ui"]["border"], "ff"),
        "editor_active_wrap_guide": alpha(p["whitespace"], "ff"),
        "editor_indent_guide": alpha(r["ui"]["border"], "66"),
        "editor_indent_guide_active": alpha(r["ui"]["borderActive"], "99"),
        "editor_document_highlight_bracket_background": alpha(r["ui"]["selection"], "33"),
        "editor_document_highlight_read_background": alpha(r["ui"]["selection"], "40"),
        "editor_document_highlight_write_background": alpha(r["ui"]["selection"], "60"),
        "terminal_background": alpha(r["ui"]["bg"], "ff"),
        "terminal_foreground": alpha(r["ui"]["fg"], "ff"),
        "terminal_bright_foreground": alpha(p["fgBright"], "ff"),
        "terminal_dim_foreground": alpha(p["fgMuted"], "ff"),
        "terminal_ansi_background": alpha(r["ui"]["bg"], "ff"),
        "terminal_ansi_black": alpha(r["ansi"]["black"], "ff"),
        "terminal_ansi_bright_black": alpha(r["ansi"]["brightBlack"], "ff"),
        "terminal_ansi_dim_black": alpha(dim_terminal(r["ansi"]["black"], r["ui"]["bg"], light=is_light), "ff"),
        "terminal_ansi_red": alpha(r["ansi"]["red"], "ff"),
        "terminal_ansi_bright_red": alpha(r["ansi"]["brightRed"], "ff"),
        "terminal_ansi_dim_red": alpha(dim_terminal(r["ansi"]["red"], r["ui"]["bg"], light=is_light), "ff"),
        "terminal_ansi_green": alpha(r["ansi"]["green"], "ff"),
        "terminal_ansi_bright_green": alpha(r["ansi"]["brightGreen"], "ff"),
        "terminal_ansi_dim_green": alpha(dim_terminal(r["ansi"]["green"], r["ui"]["bg"], light=is_light), "ff"),
        "terminal_ansi_yellow": alpha(r["ansi"]["yellow"], "ff"),
        "terminal_ansi_bright_yellow": alpha(r["ansi"]["brightYellow"], "ff"),
        "terminal_ansi_dim_yellow": alpha(dim_terminal(r["ansi"]["yellow"], r["ui"]["bg"], light=is_light), "ff"),
        "terminal_ansi_blue": alpha(r["ansi"]["blue"], "ff"),
        "terminal_ansi_bright_blue": alpha(r["ansi"]["brightBlue"], "ff"),
        "terminal_ansi_dim_blue": alpha(dim_terminal(r["ansi"]["blue"], r["ui"]["bg"], light=is_light), "ff"),
        "terminal_ansi_magenta": alpha(r["ansi"]["magenta"], "ff"),
        "terminal_ansi_bright_magenta": alpha(r["ansi"]["brightMagenta"], "ff"),
        "terminal_ansi_dim_magenta": alpha(dim_terminal(r["ansi"]["magenta"], r["ui"]["bg"], light=is_light), "ff"),
        "terminal_ansi_cyan": alpha(r["ansi"]["cyan"], "ff"),
        "terminal_ansi_bright_cyan": alpha(r["ansi"]["brightCyan"], "ff"),
        "terminal_ansi_dim_cyan": alpha(dim_terminal(r["ansi"]["cyan"], r["ui"]["bg"], light=is_light), "ff"),
        "terminal_ansi_white": alpha(r["ansi"]["white"], "ff"),
        "terminal_ansi_bright_white": alpha(r["ansi"]["brightWhite"], "ff"),
        "terminal_ansi_dim_white": alpha(dim_terminal(r["ansi"]["white"], r["ui"]["bg"], light=is_light), "ff"),
        "conflict": alpha(r["vcs"]["conflict"], "ff"),
        "conflict_background": alpha(r["vcs"]["conflict"], "1a"),
        "conflict_border": alpha(p["yellowBright"], "ff"),
        "created": alpha(r["diagnostics"]["ok"], "ff"),
        "created_background": alpha(r["diagnostics"]["ok"], "1a"),
        "created_border": alpha(p["greenBright"], "ff"),
        "deleted": alpha(r["diagnostics"]["error"], "ff"),
        "deleted_background": alpha(r["diagnostics"]["error"], "1a"),
        "deleted_border": alpha(p["pink"], "ff"),
        "error": alpha(r["diagnostics"]["error"], "ff"),
        "error_background": alpha(r["diagnostics"]["error"], "1a"),
        "error_border": alpha(p["pink"], "ff"),
        "hidden": alpha(p["lineNumber"], "ff"),
        "hidden_background": alpha(r["ui"]["bgAlt"], "ff"),
        "hidden_border": alpha(r["ui"]["border"], "ff"),
        "hint": alpha(r["diagnostics"]["hint"], "ff"),
        "hint_background": alpha(r["diagnostics"]["hint"], "1a"),
        "hint_border": alpha(p["cyanBright"], "ff"),
        "ignored": alpha(p["lineNumber"], "ff"),
        "ignored_background": alpha(r["ui"]["bgAlt"], "ff"),
        "ignored_border": alpha(r["ui"]["border"], "ff"),
        "info": alpha(r["diagnostics"]["info"], "ff"),
        "info_background": alpha(r["diagnostics"]["info"], "1a"),
        "info_border": alpha(p["blueBright"], "ff"),
        "modified": alpha(r["vcs"]["modified"], "ff"),
        "modified_background": alpha(r["vcs"]["modified"], "1a"),
        "modified_border": alpha(p["yellowBright"], "ff"),
        "predictive": alpha(p["lineNumber"], "ff"),
        "predictive_background": alpha(r["ui"]["bgAlt"], "ff"),
        "predictive_border": alpha(r["ui"]["border"], "ff"),
        "renamed": alpha(r["vcs"]["renamed"], "ff"),
        "renamed_background": alpha(r["vcs"]["renamed"], "1a"),
        "renamed_border": alpha(p["blueBright"], "ff"),
        "success": alpha(r["diagnostics"]["ok"], "ff"),
        "success_background": alpha(r["diagnostics"]["ok"], "1a"),
        "success_border": alpha(p["greenBright"], "ff"),
        "unreachable": alpha(p["lineNumber"], "ff"),
        "unreachable_background": alpha(r["ui"]["bgAlt"], "ff"),
        "unreachable_border": alpha(r["ui"]["border"], "ff"),
        "warning": alpha(r["diagnostics"]["warning"], "ff"),
        "warning_background": alpha(r["diagnostics"]["warning"], "1a"),
        "warning_border": alpha(p["yellowBright"], "ff"),
        "syntax_attribute": alpha(r["syntax"]["attribute"], "ff"),
        "syntax_boolean": alpha(r["syntax"]["constant"], "ff"),
        "syntax_comment": alpha(r["syntax"]["comment"], "ff"),
        "syntax_constant": alpha(r["syntax"]["constant"], "ff"),
        "syntax_constructor": alpha(r["syntax"]["function"], "ff"),
        "syntax_embedded": alpha(r["syntax"]["text"], "ff"),
        "syntax_enum": alpha(r["syntax"]["type"], "ff"),
        "syntax_function": alpha(r["syntax"]["function"], "ff"),
        "syntax_hint": alpha(r["diagnostics"]["hint"], "ff"),
        "syntax_keyword": alpha(r["syntax"]["keyword"], "ff"),
        "syntax_label": alpha(r["syntax"]["tag"], "ff"),
        "syntax_link_text": alpha(r["syntax"]["link"], "ff"),
        "syntax_link_uri": alpha(r["syntax"]["link"], "ff"),
        "syntax_namespace": alpha(r["syntax"]["namespace"], "ff"),
        "syntax_number": alpha(r["syntax"]["number"], "ff"),
        "syntax_operator": alpha(r["syntax"]["operator"], "ff"),
        "syntax_property": alpha(r["syntax"]["property"], "ff"),
        "syntax_punctuation": alpha(r["syntax"]["punctuation"], "ff"),
        "syntax_punctuation_bracket": alpha(r["syntax"]["punctuation"], "ff"),
        "syntax_punctuation_delimiter": alpha(r["syntax"]["punctuation"], "ff"),
        "syntax_punctuation_special": alpha(r["syntax"]["stringEscape"], "ff"),
        "syntax_string": alpha(r["syntax"]["string"], "ff"),
        "syntax_string_escape": alpha(r["syntax"]["stringEscape"], "ff"),
        "syntax_string_regex": alpha(r["syntax"]["stringEscape"], "ff"),
        "syntax_string_special": alpha(r["syntax"]["stringEscape"], "ff"),
        "syntax_string_special_symbol": alpha(r["syntax"]["constant"], "ff"),
        "syntax_tag": alpha(r["syntax"]["tag"], "ff"),
        "syntax_text_literal": alpha(r["syntax"]["string"], "ff"),
        "syntax_title": alpha(r["syntax"]["tag"], "ff"),
        "syntax_type": alpha(r["syntax"]["type"], "ff"),
        "syntax_variable": alpha(r["syntax"]["variable"], "ff"),
        "syntax_variable_special": alpha(r["syntax"]["property"], "ff"),
        "syntax_variant": alpha(r["syntax"]["function"], "ff"),
    }
    for idx, color in enumerate(players, start=1):
        slots[f"player_{idx}"] = alpha(color, "ff")
        slots[f"player_{idx}_selection"] = alpha(color, "3d")
    return slots


def zed_theme_doc(ctx: dict[str, Any], root: Path) -> dict[str, Any]:
    meta = ctx["meta"]
    appearance = meta["variant"]
    template = load_zed_template(root)
    slots = build_zed_slots(ctx)

    style: dict[str, Any] = {}
    for section in template["style_sections"]:
        for entry in section["entries"]:
            style[entry["key"]] = render_template_value(entry["value"], slots)

    syntax: dict[str, Any] = {}
    for section in template["syntax_sections"]:
        for entry in section["entries"]:
            syntax[entry["key"]] = render_template_value(entry["value"], slots)

    players = [render_template_value(player, slots) for player in template["players"]]

    override_path = root / "overrides" / "zed" / f"{meta['id']}.yaml"
    if override_path.exists():
        from validate_overrides import validate_zed_override

        validator = Validator()
        validate_zed_override(validator, override_path)
        if validator.errors:
            raise RuntimeError("\n".join(validator.errors))
        override = load_yaml(override_path)
        if (override["meta"]["theme"], override["meta"]["variant"]) != (meta["id"], appearance):
            raise RuntimeError(f"{override_path}: metadata does not match the canonical theme")
        patch = resolve_context_value(override, ctx)
        style.update(patch["style"])
        for capture, attrs in patch["syntax"].items():
            syntax[capture] = {**syntax.get(capture, {}), **attrs}
        for player, attrs in patch["players"].items():
            players[int(player.removeprefix("player_")) - 1].update(attrs)

    doc = {
        "$schema": template["theme_family_schema"]["schema_url"],
        "name": meta["name"],
        "author": meta["author"],
        "themes": [
            {
                "name": meta["name"],
                "appearance": appearance,
                "style": {**style, "players": players, "syntax": syntax},
            }
        ],
    }
    validate_zed_theme_doc(doc, root)
    return doc


def lua_string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def load_nvim_template(root: Path) -> dict[str, Any]:
    template_path = root / "templates" / "nvim" / "official-template.json"
    return load_json(template_path)


def load_nvim_plugin_template(root: Path) -> dict[str, Any]:
    template_path = root / "templates" / "nvim" / "plugins.json"
    return load_json(template_path)


def load_ghostty_template(root: Path) -> dict[str, Any]:
    template_path = root / "templates" / "ghostty" / "official-template.json"
    return load_json(template_path)


def load_starship_template(root: Path) -> dict[str, Any]:
    template_path = root / "templates" / "starship" / "official-template.json"
    return load_json(template_path)


def load_tmux_template(root: Path) -> dict[str, Any]:
    template_path = root / "templates" / "tmux" / "official-template.json"
    return load_json(template_path)


def load_k9s_template(root: Path) -> dict[str, Any]:
    template_path = root / "templates" / "k9s" / "official-template.json"
    return load_json(template_path)


def load_zellij_template(root: Path) -> dict[str, Any]:
    template_path = root / "templates" / "zellij" / "official-template.json"
    return load_json(template_path)


def load_pi_template(root: Path) -> dict[str, Any]:
    template_path = root / "templates" / "pi" / "official-template.json"
    return load_json(template_path)


def load_zellij_override(root: Path, theme_id: str) -> dict[str, Any] | None:
    path = root / "overrides" / "zellij" / f"{theme_id}.yaml"
    if not path.exists():
        return None
    return load_yaml(path)


def load_nvim_override(root: Path, theme_id: str) -> dict[str, Any] | None:
    path = root / "overrides" / "nvim" / f"{theme_id}.yaml"
    if not path.exists():
        return None
    return load_yaml(path)


def load_ghostty_override(root: Path, theme_id: str) -> dict[str, Any] | None:
    from validate_overrides import validate_ghostty_override

    path = root / "overrides" / "ghostty" / f"{theme_id}.yaml"
    if not path.exists():
        return None
    validator = Validator()
    validate_ghostty_override(validator, path)
    if validator.errors:
        raise RuntimeError("\n".join(validator.errors))
    return load_yaml(path)


def apply_nvim_override_sections(
    sections: list[dict[str, Any]],
    override: dict[str, Any] | None,
    ctx: dict[str, Any],
    *,
    append_extra: bool = True,
) -> list[dict[str, Any]]:
    if not override:
        return sections

    group_overrides = override.get("groups", {})
    links = override.get("links", {})

    if not isinstance(group_overrides, dict):
        raise RuntimeError("nvim override groups must be a mapping")
    if not isinstance(links, dict):
        raise RuntimeError("nvim override links must be a mapping")

    out: list[dict[str, Any]] = []
    seen_groups: set[str] = set()

    for section in sections:
        new_section = {**section, "groups": []}
        for spec in section["groups"]:
            group = spec["group"]
            attrs = dict(spec["attrs"])

            if group in group_overrides:
                patch = group_overrides[group]
                if not isinstance(patch, dict):
                    raise RuntimeError(f"nvim override for group {group!r} must be a mapping")
                attrs.update(resolve_context_value(patch, ctx))

            if group in links:
                attrs = {"link": resolve_context_value(links[group], ctx)}

            new_section["groups"].append({"group": group, "attrs": attrs})
            seen_groups.add(group)
        out.append(new_section)

    extra_groups: list[dict[str, Any]] = []
    for group, patch in group_overrides.items():
        if group in seen_groups:
            continue
        if not isinstance(patch, dict):
            raise RuntimeError(f"nvim override for group {group!r} must be a mapping")
        extra_groups.append({"group": group, "attrs": resolve_context_value(patch, ctx)})

    for group, target in links.items():
        if group in seen_groups or group in group_overrides:
            continue
        extra_groups.append({"group": group, "attrs": {"link": resolve_context_value(target, ctx)}})

    if append_extra and extra_groups:
        out.append(
            {
                "name": "Override groups",
                "source": "themes/overrides/nvim",
                "groups": extra_groups,
            }
        )

    return out


def nvim_lua(ctx: dict[str, Any], root: Path) -> str:
    meta = ctx["meta"]
    p = ctx["palette"]
    r = ctx["roles"]
    colors_name = meta["id"].replace("-", "_")
    background = meta["variant"]
    is_light = background == "light"
    nvim_template = load_nvim_template(root)
    nvim_plugin_template = load_nvim_plugin_template(root)
    nvim_override = load_nvim_override(root, meta["id"])

    palette_table = {
        "bg": r["ui"]["bg"],
        "bg_float": r["ui"]["bgElevated"],
        "bg_alt": r["ui"]["bgAlt"],
        "border": r["ui"]["border"],
        "hover": raised_surface(r["ui"]["bgAlt"], r["ui"]["border"], light=is_light),
        "selection": mix(r["ui"]["selection"], r["ui"]["bg"], 0.42 if is_light else 0.16),
        "search": soft_background(r["ui"]["search"], r["ui"]["bg"], light=is_light, strength=0.68),
        "search_active": sharpen(r["ui"]["borderActive"], r["ui"]["bg"], light=is_light),
        "fg": r["ui"]["fg"],
        "fg_muted": r["ui"]["fgMuted"],
        "fg_inactive": r["ui"]["fgInactive"],
        "fg_bright": r["ui"]["currentLineNumber"],
        "comment": r["syntax"]["comment"],
        "ghost_text": mix(r["syntax"]["comment"], p["blue"], 0.42 if is_light else 0.42),
        "red": p["red"],
        "orange": p["orange"],
        "yellow": p["yellow"],
        "green": p["green"],
        "cyan": p["cyan"],
        "blue": p["blue"],
        "magenta": p["magenta"],
        "pink": p["pink"],
        "linenr": p["lineNumber"],
        "syntax_text": r["syntax"]["text"],
        "syntax_comment": r["syntax"]["comment"],
        "syntax_string": r["syntax"]["string"],
        "syntax_string_escape": r["syntax"]["stringEscape"],
        "syntax_number": r["syntax"]["number"],
        "syntax_constant": r["syntax"]["constant"],
        "syntax_keyword": r["syntax"]["keyword"],
        "syntax_operator": r["syntax"]["operator"],
        "syntax_variable": r["syntax"]["variable"],
        "syntax_parameter": r["syntax"]["parameter"],
        "syntax_property": r["syntax"]["property"],
        "syntax_field": r["syntax"]["field"],
        "syntax_func": r["syntax"]["function"],
        "syntax_method": r["syntax"]["method"],
        "syntax_type": r["syntax"]["type"],
        "syntax_class": r["syntax"]["class"],
        "syntax_interface": r["syntax"]["interface"],
        "syntax_namespace": r["syntax"]["namespace"],
        "syntax_builtin": r["syntax"]["builtin"],
        "syntax_tag": r["syntax"]["tag"],
        "syntax_attribute": r["syntax"]["attribute"],
        "syntax_punctuation": r["syntax"]["punctuation"],
        "syntax_link": r["syntax"]["link"],
        "diff_add_bg": soft_background(r["vcs"]["added"], r["ui"]["bg"], light=is_light, strength=0.82),
        "diff_change_bg": soft_background(r["vcs"]["modified"], r["ui"]["bg"], light=is_light, strength=0.82),
        "diff_delete_bg": soft_background(r["vcs"]["removed"], r["ui"]["bg"], light=is_light, strength=0.82),
        "diff_text_bg": soft_background(r["ui"]["search"], r["ui"]["bg"], light=is_light, strength=0.74),
        "cursor": r["ui"]["cursor"],
        "none": "NONE",
    }

    def lua_value(value: Any) -> str:
        if isinstance(value, str) and value.startswith("$"):
            return f"p.{value[1:]}"
        if isinstance(value, bool):
            return "true" if value else "false"
        return lua_string(value)

    def lua_opts(opts: dict[str, Any]) -> str:
        return "{ " + ", ".join(f"{key} = {lua_value(value)}" for key, value in opts.items()) + " }"

    lines = [
        f"-- Auto-generated from themes/core/{meta['id']}.yaml",
        f"-- Template: {nvim_template['name']} v{nvim_template['version']}",
        "local M = {}",
        "",
        "local p = {",
    ]
    for key, value in palette_table.items():
        lines.append(f"  {key} = {lua_string(value)},")
    lines.extend(
        [
            "}",
            "",
            "function M.setup()",
            '  vim.cmd("hi clear")',
            '  if vim.fn.exists("syntax_on") then',
            '    vim.cmd("syntax reset")',
            "  end",
            "  vim.o.termguicolors = true",
            f"  vim.o.background = {lua_string(background)}",
            f"  vim.g.colors_name = {lua_string(colors_name)}",
            "",
            "  local hl = function(group, opts)",
            "    vim.api.nvim_set_hl(0, group, opts)",
            "  end",
        ]
    )

    rendered_sections = apply_nvim_override_sections(nvim_template["sections"], nvim_override, ctx)
    rendered_plugin_sections = apply_nvim_override_sections(
        nvim_plugin_template["sections"], nvim_override, ctx, append_extra=False
    )

    for section in rendered_sections:
        lines.append("")
        lines.append(f"  -- {section['name']}")
        for spec in section["groups"]:
            lines.append(f"  hl({lua_string(spec['group'])}, {lua_opts(spec['attrs'])})")

    for section in rendered_plugin_sections:
        lines.append("")
        lines.append(f"  -- Plugin: {section['name']}")
        for spec in section["groups"]:
            lines.append(f"  hl({lua_string(spec['group'])}, {lua_opts(spec['attrs'])})")

    lines.extend(["end", "", "return M"])
    return "\n".join(lines) + "\n"


def build_ghostty_slots(ctx: dict[str, Any]) -> dict[str, str]:
    p = ctx["palette"]
    r = ctx["roles"]
    cursor_text = best_contrast(r["ui"]["cursor"], p["white"], r["ui"]["bg"], r["ui"]["fg"])
    palette = {
        0: r["ansi"]["black"],
        1: r["ansi"]["red"],
        2: r["ansi"]["green"],
        3: r["ansi"]["yellow"],
        4: r["ansi"]["blue"],
        5: r["ansi"]["magenta"],
        6: r["ansi"]["cyan"],
        7: r["ansi"]["white"],
        8: r["ansi"]["brightBlack"],
        9: r["ansi"]["brightRed"],
        10: r["ansi"]["brightGreen"],
        11: r["ansi"]["brightYellow"],
        12: r["ansi"]["brightBlue"],
        13: r["ansi"]["brightMagenta"],
        14: r["ansi"]["brightCyan"],
        15: r["ansi"]["brightWhite"],
    }
    slots = {
        "background": r["ui"]["bg"],
        "foreground": r["ui"]["fg"],
        "selection_background": r["ui"]["selection"],
        "selection_foreground": best_contrast(r["ui"]["selection"], p["white"], r["ui"]["bg"], r["ui"]["fg"]),
        "cursor_color": r["ui"]["cursor"],
        "cursor_text": cursor_text,
        "faint_opacity": "0.5",
    }
    for idx, color in palette.items():
        slots[f"palette_{idx}_entry"] = f"{idx}={color}"
    return slots


def apply_ghostty_override_slots(
    slots: dict[str, str], override: dict[str, Any] | None, ctx: dict[str, Any]
) -> dict[str, str]:
    if not override:
        return slots

    slot_overrides = override.get("slots", {})
    if not isinstance(slot_overrides, dict):
        raise RuntimeError("ghostty override slots must be a mapping")

    out = dict(slots)
    for key, value in slot_overrides.items():
        rendered = resolve_context_value(value, ctx)
        if not isinstance(rendered, str):
            raise RuntimeError(f"ghostty override slot {key!r} must resolve to a string")
        out[key] = rendered
    return out


def ghostty_theme_conf(ctx: dict[str, Any], root: Path) -> str:
    template = load_ghostty_template(root)
    override = load_ghostty_override(root, ctx["meta"]["id"])
    if override and (override["meta"]["theme"], override["meta"]["variant"]) != (ctx["meta"]["id"], ctx["meta"]["variant"]):
        raise RuntimeError("Ghostty override metadata does not match the canonical theme")
    slots = apply_ghostty_override_slots(build_ghostty_slots(ctx), override, ctx)
    lines = [
        f"# Auto-generated from themes/core/{ctx['meta']['id']}.yaml",
        f"# Template: {template['name']} v{template['version']}",
    ]
    for section in template["sections"]:
        if lines[-1].startswith("#") is False:
            lines.append("")
        for entry in section["entries"]:
            value = render_template_value(entry["value"], slots)
            lines.append(f"{entry['key']} = {value}")
    return "\n".join(lines) + "\n"


def build_starship_slots(ctx: dict[str, Any]) -> dict[str, Any]:
    p = ctx["palette"]
    r = ctx["roles"]
    light = ctx["meta"]["variant"] == "light"
    layer1 = r["ui"]["bgElevated"]
    layer2 = mix(r["ui"]["bgAlt"], r["ui"]["border"], 0.5 if light else 0.35)
    layer3 = p["whitespace"]
    return {
        "format": """[](iris)\\
[ 󰉋 ](bg:iris fg:base)\\
[](bg:foam fg:iris)\\
$directory\\
[](fg:foam bg:layer3)\\
$git_branch\\
$git_status\\
[](fg:layer3 bg:layer2)\\
$c\\
$cpp\\
$rust\\
$golang\\
$nodejs\\
$php\\
$java\\
$kotlin\\
$haskell\\
$python\\
[](fg:layer2 bg:layer1)\\
$docker_context\\
$conda\\
$pixi\\
[](fg:layer1)\\
$fill\\
[](fg:layer1)\\
$kubernetes\\
[](fg:layer2 bg:layer1)\\
$hostname\\
[](fg:layer3 bg:layer2)\\
$time\\
[](fg:layer3)
$character""",
        "palette_name": ctx["meta"]["id"].replace("-", "_"),
        "base": r["ui"]["bg"],
        "surface": r["ui"]["bgAlt"],
        "overlay": layer3,
        "muted": r["ui"]["fgMuted"],
        "subtle": r["ui"]["fg"],
        "text": r["ui"]["fg"],
        "love": r["diagnostics"]["error"],
        "gold": r["diagnostics"]["warning"],
        "rose": p["pink"],
        "pine": r["syntax"]["string"],
        "foam": r["syntax"]["stringEscape"],
        "iris": r["syntax"]["keyword"],
        "highlight_low": layer1,
        "highlight_med": layer2,
        "highlight_high": layer3,
        "green": p["green"],
        "layer1": layer1,
        "layer2": layer2,
        "layer3": layer3,
    }


def toml_literal(value: Any) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int) or isinstance(value, float):
        return str(value)
    if isinstance(value, str) and "\n" in value:
        return '"""\n' + value + '"""'
    return json.dumps(str(value), ensure_ascii=False)


def lua_literal(value: Any, indent: int = 0) -> str:
    if isinstance(value, str):
        return lua_string(value)
    if isinstance(value, bool):
        return "true" if value else "false"
    if value is None:
        return "nil"
    if isinstance(value, int) or isinstance(value, float):
        return str(value)
    if isinstance(value, list):
        if not value:
            return "{}"
        inner_indent = " " * (indent + 2)
        closing_indent = " " * indent
        lines = [f"{inner_indent}{lua_literal(item, indent + 2)}," for item in value]
        return "{\n" + "\n".join(lines) + f"\n{closing_indent}" + "}"
    if isinstance(value, dict):
        if not value:
            return "{}"
        inner_indent = " " * (indent + 2)
        closing_indent = " " * indent
        lines = [f"{inner_indent}{key} = {lua_literal(item, indent + 2)}," for key, item in value.items()]
        return "{\n" + "\n".join(lines) + f"\n{closing_indent}" + "}"
    raise TypeError(f"unsupported Lua literal value: {value!r}")


def render_yaml(data: dict[str, Any]) -> str:
    def yaml_scalar(value: Any) -> str:
        if isinstance(value, bool):
            return "true" if value else "false"
        if value is None:
            return "null"
        if isinstance(value, int) or isinstance(value, float):
            return str(value)
        return json.dumps(str(value), ensure_ascii=False)

    def yaml_lines(value: Any, indent: int = 0) -> list[str]:
        prefix = " " * indent
        if isinstance(value, dict):
            lines: list[str] = []
            for key, item in value.items():
                if isinstance(item, dict) or isinstance(item, list):
                    lines.append(f"{prefix}{key}:")
                    lines.extend(yaml_lines(item, indent + 2))
                else:
                    lines.append(f"{prefix}{key}: {yaml_scalar(item)}")
            return lines
        if isinstance(value, list):
            lines = []
            for item in value:
                if isinstance(item, dict) or isinstance(item, list):
                    lines.append(f"{prefix}-")
                    lines.extend(yaml_lines(item, indent + 2))
                else:
                    lines.append(f"{prefix}- {yaml_scalar(item)}")
            return lines
        return [f"{prefix}{yaml_scalar(value)}"]

    return "\n".join(yaml_lines(data)) + "\n"


def starship_theme_toml(ctx: dict[str, Any], root: Path) -> str:
    template = load_starship_template(root)
    slots = build_starship_slots(ctx)
    module_overrides = {}
    override_path = root / "overrides" / "starship" / f"{ctx['meta']['id']}.yaml"
    if override_path.exists():
        from validate_overrides import validate_starship_override

        validator = Validator()
        validate_starship_override(validator, override_path)
        if validator.errors:
            raise RuntimeError("\n".join(validator.errors))
        override = load_yaml(override_path)
        if (override["meta"]["theme"], override["meta"]["variant"]) != (ctx["meta"]["id"], ctx["meta"]["variant"]):
            raise RuntimeError(f"{override_path}: metadata does not match the canonical theme")
        slots.update(resolve_context_value(override["slots"], ctx))
        module_overrides = override["modules"]
    palette = template["palette"]
    palette_name = render_template_value(palette["name"], slots)

    lines = [
        f"# Auto-generated from themes/core/{ctx['meta']['id']}.yaml",
        f"# Template: {template['name']} v{template['version']}",
        '"$schema" = "https://starship.rs/config-schema.json"',
        "",
        f"palette = {json.dumps(palette_name, ensure_ascii=False)}",
        f"format = {toml_literal(render_template_value(template['format'], slots))}",
    ]
    for section in template["sections"]:
        lines.extend(["", f"[{section['table']}]"])
        for entry in section["entries"]:
            value = module_overrides.get(section["table"], {}).get(entry["key"], entry["value"])
            value = render_template_value(value, slots)
            lines.append(f"{json.dumps(entry['key'], ensure_ascii=False)} = {toml_literal(value)}")
    lines.extend(["", f"[palettes.{palette_name}]"])
    for entry in palette["entries"]:
        value = render_template_value(entry["value"], slots)
        lines.append(f"{entry['key']} = {json.dumps(value, ensure_ascii=False)}")
    return "\n".join(lines) + "\n"

def build_k9s_slots(ctx: dict[str, Any]) -> dict[str, Any]:
    p = ctx["palette"]
    r = ctx["roles"]
    is_light = ctx["meta"]["variant"] == "light"
    selected_bg = ensure_distinct_background(r["ui"]["selection"], r["ui"]["bg"], r["ui"]["borderActive"], light=is_light)
    selected_fg = best_contrast(selected_bg, p["white"], r["ui"]["bg"], r["ui"]["fg"])
    dialog_bg = raised_surface(r["ui"]["bgAlt"], r["ui"]["border"], light=is_light)
    dialog_button_bg = r["syntax"]["function"]
    dialog_button_focus_bg = r["vcs"]["conflict"]
    dialog_button_fg = best_contrast(dialog_button_bg, p["white"], r["ui"]["bg"], r["ui"]["fg"])
    dialog_button_focus_fg = best_contrast(dialog_button_focus_bg, p["white"], r["ui"]["bg"], r["ui"]["fg"])
    return {
        "body_fg": r["ui"]["fg"],
        "body_bg": r["ui"]["bg"],
        "body_logo": r["syntax"]["function"],
        "prompt_fg": r["ui"]["fg"],
        "prompt_bg": r["ui"]["bg"],
        "prompt_suggest": r["syntax"]["function"],
        "help_fg": r["ui"]["fg"],
        "help_bg": r["ui"]["bg"],
        "help_section": r["diagnostics"]["ok"],
        "help_key": r["syntax"]["link"],
        "help_num_key": r["diagnostics"]["error"],
        "info_fg": r["diagnostics"]["info"],
        "info_section": r["ui"]["fg"],
        "dialog_fg": r["ui"]["fg"],
        "dialog_bg": dialog_bg,
        "dialog_button_fg": dialog_button_fg,
        "dialog_button_bg": dialog_button_bg,
        "dialog_button_focus_fg": dialog_button_focus_fg,
        "dialog_button_focus_bg": dialog_button_focus_bg,
        "dialog_label": r["diagnostics"]["warning"],
        "dialog_field": r["ui"]["fg"],
        "frame_border_fg": r["ui"]["border"],
        "frame_border_focus": r["ui"]["borderActive"],
        "frame_menu_fg": r["ui"]["fg"],
        "frame_menu_key": r["syntax"]["link"],
        "frame_menu_num_key": r["vcs"]["conflict"],
        "frame_crumbs_fg": r["ui"]["fg"],
        "frame_crumbs_bg": r["ui"]["bgAlt"],
        "frame_crumbs_active": selected_bg,
        "frame_status_new": r["diagnostics"]["info"],
        "frame_status_modify": r["vcs"]["modified"],
        "frame_status_add": r["vcs"]["added"],
        "frame_status_pending": r["diagnostics"]["warning"],
        "frame_status_error": r["diagnostics"]["error"],
        "frame_status_highlight": r["syntax"]["link"],
        "frame_status_kill": r["vcs"]["conflict"],
        "frame_status_completed": r["ui"]["fgMuted"],
        "frame_title_fg": r["ui"]["fg"],
        "frame_title_bg": r["ui"]["bgAlt"],
        "frame_title_highlight": r["vcs"]["modified"],
        "frame_title_counter": r["diagnostics"]["info"],
        "frame_title_filter": r["vcs"]["conflict"],
        "table_fg": r["ui"]["fg"],
        "table_bg": r["ui"]["bg"],
        "table_cursor_fg": selected_fg,
        "table_cursor_bg": selected_bg,
        "table_mark": r["diagnostics"]["warning"],
        "table_header_fg": r["ui"]["fg"],
        "table_header_bg": r["ui"]["bg"],
        "table_header_sorter": r["syntax"]["link"],
        "xray_fg": r["ui"]["fg"],
        "xray_bg": r["ui"]["bg"],
        "xray_cursor": selected_bg,
        "xray_cursor_text": selected_fg,
        "xray_graphic": r["syntax"]["link"],
        "charts_bg": r["ui"]["bg"],
        "charts_chart_bg": r["ui"]["bg"],
        "charts_dial_bg": r["ui"]["bg"],
        "charts_dial_primary": r["vcs"]["added"],
        "charts_dial_secondary": r["diagnostics"]["error"],
        "charts_chart_primary": r["vcs"]["added"],
        "charts_chart_secondary": r["diagnostics"]["error"],
        "charts_cpu_primary": r["syntax"]["keyword"],
        "charts_cpu_secondary": r["diagnostics"]["info"],
        "charts_mem_primary": r["diagnostics"]["warning"],
        "charts_mem_secondary": r["vcs"]["conflict"],
        "yaml_key": r["syntax"]["link"],
        "yaml_colon": r["ui"]["fgMuted"],
        "yaml_value": r["ui"]["fg"],
        "logs_fg": r["ui"]["fg"],
        "logs_bg": r["ui"]["bg"],
        "logs_indicator_fg": dialog_button_fg,
        "logs_indicator_bg": dialog_button_bg,
        "logs_indicator_on": r["vcs"]["added"],
        "logs_indicator_off": r["ui"]["fgMuted"],
    }


def k9s_theme_doc(ctx: dict[str, Any], root: Path) -> dict[str, Any]:
    template = load_k9s_template(root)
    return render_template_value(template["document"], build_k9s_slots(ctx))


def build_pi_slots(ctx: dict[str, Any]) -> dict[str, str]:
    p = ctx["palette"]
    r = ctx["roles"]
    is_light = ctx["meta"]["variant"] == "light"
    bg = r["ui"]["bg"]
    return {
        "theme_id": ctx["meta"]["id"],
        "accent": r["syntax"]["link"],
        "border": r["ui"]["fgMuted"],
        "border_accent": r["ui"]["borderActive"],
        "border_muted": r["ui"]["border"],
        "success": r["diagnostics"]["ok"],
        "error": r["diagnostics"]["error"],
        "warning": r["diagnostics"]["warning"],
        "muted": r["ui"]["fgInactive"],
        "dim": r["syntax"]["comment"],
        "text": r["ui"]["fg"],
        "thinking_text": r["ui"]["fgMuted"],
        "selected_bg": r["ui"]["selection"],
        "scrollbar_track": r["ui"]["border"],
        "scrollbar_thumb": r["ui"]["fgMuted"],
        "search_match_bg": r["ui"]["search"],
        "search_match_text": r["ui"]["searchText"],
        "user_message_bg": r["ui"]["bgAlt"],
        "user_message_text": r["ui"]["fg"],
        "custom_message_bg": soft_background(p["magenta"], bg, light=is_light, strength=0.9),
        "custom_message_text": r["ui"]["fg"],
        "custom_message_label": p["magenta"],
        "tool_pending_bg": r["ui"]["bgElevated"],
        "tool_success_bg": soft_background(r["diagnostics"]["ok"], bg, light=is_light, strength=0.9),
        "tool_error_bg": soft_background(r["diagnostics"]["error"], bg, light=is_light, strength=0.9),
        "tool_title": r["syntax"]["link"],
        "tool_output": r["ui"]["fgInactive"],
        "md_heading": r["syntax"]["constant"],
        "md_link": r["syntax"]["link"],
        "md_link_url": r["ui"]["fgMuted"],
        "md_code": r["syntax"]["builtin"],
        "md_code_block": r["syntax"]["text"],
        "md_code_block_border": r["ui"]["border"],
        "md_quote": r["ui"]["fgInactive"],
        "md_quote_border": r["ui"]["fgMuted"],
        "md_hr": r["ui"]["border"],
        "md_list_bullet": r["diagnostics"]["ok"],
        "tool_diff_added": r["vcs"]["added"],
        "tool_diff_removed": r["vcs"]["removed"],
        "tool_diff_context": r["ui"]["fgInactive"],
        "syntax_comment": r["syntax"]["comment"],
        "syntax_keyword": r["syntax"]["keyword"],
        "syntax_function": r["syntax"]["function"],
        "syntax_variable": r["syntax"]["variable"],
        "syntax_string": r["syntax"]["string"],
        "syntax_number": r["syntax"]["number"],
        "syntax_type": r["syntax"]["type"],
        "syntax_operator": r["syntax"]["operator"],
        "syntax_punctuation": r["syntax"]["punctuation"],
        "thinking_off": r["ui"]["border"],
        "thinking_minimal": r["ui"]["fgMuted"],
        "thinking_low": p["blue"],
        "thinking_medium": p["cyan"],
        "thinking_high": p["magenta"],
        "thinking_xhigh": p["red"],
        "thinking_max": p["orange"],
        "bash_mode": r["diagnostics"]["ok"],
        "page_bg": r["ui"]["bg"],
        "card_bg": r["ui"]["bgAlt"],
        "info_bg": soft_background(r["diagnostics"]["warning"], bg, light=is_light, strength=0.9),
    }


def pi_theme_doc(ctx: dict[str, Any], root: Path) -> dict[str, Any]:
    template = load_pi_template(root)
    return render_template_value(template["document"], build_pi_slots(ctx))


def herdr_theme_toml(ctx: dict[str, Any], root: Path) -> str:
    template = load_json(root / "templates" / "herdr" / "official-template.json")
    slots = {
        f"{group}_{name}": value
        for group, roles in ctx["roles"].items()
        for name, value in roles.items()
    }
    slots.update({f"palette_{name}": value for name, value in ctx["palette"].items()})
    ui = ctx["roles"]["ui"]
    # Fine steps choose the least prominent shade that meets row visibility targets.
    for step in range(1001):
        candidate = mix(ui["sidebarBg"], ui["fg"], step / 1000)
        if (
            contrast_ratio(candidate, ui["sidebarBg"]) >= 1.8
            and contrast_ratio(ui["fg"], candidate) >= 4.5
        ):
            slots["active_row_bg"] = candidate
            break
    else:
        raise RuntimeError(f"{ctx['meta']['id']}: Herdr active row cannot meet visibility and text contrast targets")
    # Leave margin above Ghostty's 1.6 minimum for its Metal color-space conversion.
    green = ctx["palette"]["green"]
    status_backgrounds = [ui["sidebarBg"], slots["active_row_bg"]]
    for step in range(101):
        candidate = green if step == 0 else mix(green, ui["fg"], step / 100)
        if all(contrast_ratio(candidate, bg) >= 1.8 for bg in status_backgrounds):
            slots["running_green"] = candidate
            break
    else:
        raise RuntimeError(f"{ctx['meta']['id']}: Herdr running green cannot reach 1.8:1 contrast")
    backgrounds = [ui[key] for key in ("bg", "panelBg", "sidebarBg", "bgAlt")]
    backgrounds.append(slots["active_row_bg"])
    # Herdr uses panel_bg as selected tab text and accent as its background.
    for slot, foreground, surfaces in (
        ("muted_text", ui["fgInactive"], backgrounds),
        ("focused_space_text", ctx["palette"]["magenta"], backgrounds),
        ("accent", ui["borderActive"], [ui["panelBg"]]),
    ):
        for step in range(101):
            candidate = mix(foreground, ui["fg"], step / 100)
            if all(contrast_ratio(candidate, bg) >= 4.5 for bg in surfaces):
                slots[slot] = candidate
                break
        else:
            raise RuntimeError(f"{ctx['meta']['id']}: Herdr {slot} cannot reach 4.5:1 contrast")
    lines = [
        f"# Auto-generated from themes/core/{ctx['meta']['id']}.yaml",
        f"# Template: {template['name']} v{template['version']}",
    ]
    for section in template["sections"]:
        lines.extend(["", f"[{section['table']}]"])
        for entry in section["entries"]:
            value = render_template_value(entry["value"], slots)
            lines.append(f"{entry['key']} = {toml_literal(value)}")
    return "\n".join(lines) + "\n"


def tmux_style(*, fg: str, bg: str | None = None, bold: bool = False) -> str:
    parts = [f"#[fg={fg}]"]
    if bg is not None:
        parts.append(f"#[bg={bg}]")
    if bold:
        parts.append("#[bold]")
    return "".join(parts)


def tmux_quote(value: str) -> str:
    return "'" + value.replace("'", "'\\''") + "'"


def build_tmux_slots(ctx: dict[str, Any]) -> dict[str, str]:
    p = ctx["palette"]
    r = ctx["roles"]
    status_bg = r["ui"]["statuslineBg"]
    status_fg = r["ui"]["statuslineFg"]
    session_bg = r["ui"]["borderActive"]
    prefix_bg = r["diagnostics"]["error"]
    sync_bg = r["diagnostics"]["warning"]
    current_index_fg = session_bg
    session_fg = best_contrast(session_bg, p["white"], r["ui"]["bg"], r["ui"]["fg"])
    prefix_fg = best_contrast(prefix_bg, p["white"], r["ui"]["bg"], r["ui"]["fg"])
    sync_fg = best_contrast(sync_bg, p["white"], r["ui"]["bg"], r["ui"]["fg"])
    normal_session = f"{tmux_style(fg=session_fg, bg=session_bg, bold=True)} #S #[default]"
    prefix_session = f"{tmux_style(fg=prefix_fg, bg=prefix_bg, bold=True)} #S #[default]"
    status_left = (
        f"#{{?client_prefix,{prefix_session},{normal_session}}}"
        f"#{{?pane_synchronized,{tmux_style(fg=sync_fg, bg=sync_bg, bold=True)} SYNC #[default],}}"
    )
    status_right = (
        f'#{{?client_prefix,{tmux_style(fg=status_fg)} '
        '? help | w tree | s sessions | #, rename | %%%% side split | " stack split | C-hjkl resize | z zoom | [ copy #[default],}'
    )
    agent_status = '#(tmux-agent-window-status #{window_id})'
    window_status = f"{tmux_style(fg=r['ui']['fgMuted'], bg=status_bg)} #I #W#F {agent_status} #[default]"
    current_window = (
        f"{tmux_style(fg=current_index_fg, bg=status_bg, bold=True)} #I "
        f"{tmux_style(fg=status_fg, bg=status_bg, bold=True)}#W#F {agent_status} #[default]"
    )
    return {
        "status_style": f"fg={status_fg},bg={status_bg}",
        "status_left": status_left,
        "status_right": status_right,
        "window_status_format": window_status,
        "window_status_current_format": current_window,
    }


def tmux_theme_conf(ctx: dict[str, Any], root: Path) -> str:
    template = load_tmux_template(root)
    slots = build_tmux_slots(ctx)
    lines = [
        f"# Auto-generated from themes/core/{ctx['meta']['id']}.yaml",
        f"# Template: {template['name']} v{template['version']}",
    ]
    for section in template["sections"]:
        lines.append("")
        for entry in section["entries"]:
            value = render_template_value(entry["value"], slots)
            command = "set -g" if entry["scope"] == "set" else "setw -g"
            lines.append(f"{command} {entry['option']} {tmux_quote(value)}")
    return "\n".join(lines) + "\n"


def zellij_rgb(hex_color: str) -> str:
    r, g, b = hex_to_rgb(hex_color)
    return f"{r} {g} {b}"


def build_zellij_player_colors(ctx: dict[str, Any], *, light: bool) -> list[str]:
    p = ctx["palette"]
    r = ctx["roles"]
    bg = r["ui"]["bg"]
    fg = r["ui"]["fg"]
    candidates = [
        r["ansi"]["brightRed"],
        r["ansi"]["brightBlue"],
        r["ansi"]["brightGreen"],
        r["ansi"]["brightYellow"],
        r["ansi"]["brightMagenta"],
        r["ansi"]["brightCyan"],
        r["diagnostics"]["error"],
        r["diagnostics"]["warning"],
        r["diagnostics"]["hint"],
        r["diagnostics"]["ok"],
        r["syntax"]["tag"],
        r["syntax"]["type"],
        p["orange"],
        p["pink"],
    ]
    colors: list[str] = []
    for candidate in candidates:
        color = candidate
        if color_distance(color, bg) < 88:
            color = sharpen(color, bg, light=light)
        if any(color_distance(color, existing) < 52 for existing in colors):
            color = mix(color, fg, 0.18 if light else 0.12)
        if color_distance(color, bg) < 88:
            color = mix(color, fg, 0.28 if light else 0.22)
        if any(color_distance(color, existing) < 52 for existing in colors):
            continue
        colors.append(color)
        if len(colors) == 10:
            break
    while len(colors) < 10:
        seed = candidates[len(colors) % len(candidates)]
        variant = mix(seed, fg, 0.32 if light else 0.18)
        if color_distance(variant, bg) < 88:
            variant = sharpen(variant, bg, light=light)
        colors.append(variant)
    return colors[:10]


def build_zellij_slots(ctx: dict[str, Any]) -> dict[str, str]:
    p = ctx["palette"]
    r = ctx["roles"]
    is_light = ctx["meta"]["variant"] == "light"
    selected_bg = ensure_distinct_background(
        r["ui"]["selection"],
        r["ui"]["panelBg"],
        r["syntax"]["link"],
        light=is_light,
    )
    selected_base = best_contrast(selected_bg, p["white"], r["ui"]["fg"], r["ui"]["bg"])
    flat_bg = r["ui"]["bg"]
    players = build_zellij_player_colors(ctx, light=is_light)
    return {
        "text_base": r["ui"]["fg"],
        "text_background": r["ui"]["panelBg"],
        "text_emphasis_0": r["diagnostics"]["warning"],
        "text_emphasis_1": r["syntax"]["link"],
        "text_emphasis_2": r["diagnostics"]["ok"],
        "text_emphasis_3": r["diagnostics"]["error"],
        "text_selected_base": selected_base,
        "text_selected_background": selected_bg,
        "text_selected_emphasis_0": r["diagnostics"]["warning"],
        "text_selected_emphasis_1": p["blueBright"],
        "text_selected_emphasis_2": p["greenBright"],
        "text_selected_emphasis_3": r["diagnostics"]["error"],
        "ribbon_selected_base": r["ui"]["bg"],
        "ribbon_selected_background": r["diagnostics"]["ok"],
        "ribbon_selected_emphasis_0": p["yellowBright"],
        "ribbon_selected_emphasis_1": r["diagnostics"]["warning"],
        "ribbon_selected_emphasis_2": r["diagnostics"]["error"],
        "ribbon_selected_emphasis_3": r["syntax"]["link"],
        "ribbon_unselected_base": p["white"],
        "ribbon_unselected_background": p["blackBright"],
        "ribbon_unselected_emphasis_0": r["vcs"]["removed"],
        "ribbon_unselected_emphasis_1": p["white"],
        "ribbon_unselected_emphasis_2": r["syntax"]["link"],
        "ribbon_unselected_emphasis_3": r["diagnostics"]["error"],
        "table_title_base": r["diagnostics"]["ok"],
        "table_title_background": flat_bg,
        "table_title_emphasis_0": r["diagnostics"]["warning"],
        "table_title_emphasis_1": r["syntax"]["link"],
        "table_title_emphasis_2": r["diagnostics"]["ok"],
        "table_title_emphasis_3": r["diagnostics"]["error"],
        "table_cell_selected_base": selected_base,
        "table_cell_selected_background": selected_bg,
        "table_cell_selected_emphasis_0": r["diagnostics"]["warning"],
        "table_cell_selected_emphasis_1": p["blueBright"],
        "table_cell_selected_emphasis_2": p["greenBright"],
        "table_cell_selected_emphasis_3": r["diagnostics"]["error"],
        "table_cell_unselected_base": r["ui"]["fg"],
        "table_cell_unselected_background": r["ui"]["panelBg"],
        "table_cell_unselected_emphasis_0": r["diagnostics"]["warning"],
        "table_cell_unselected_emphasis_1": r["syntax"]["link"],
        "table_cell_unselected_emphasis_2": r["diagnostics"]["ok"],
        "table_cell_unselected_emphasis_3": r["diagnostics"]["error"],
        "list_selected_base": selected_base,
        "list_selected_background": selected_bg,
        "list_selected_emphasis_0": r["diagnostics"]["warning"],
        "list_selected_emphasis_1": p["blueBright"],
        "list_selected_emphasis_2": p["greenBright"],
        "list_selected_emphasis_3": r["diagnostics"]["error"],
        "list_unselected_base": r["ui"]["fg"],
        "list_unselected_background": r["ui"]["panelBg"],
        "list_unselected_emphasis_0": r["diagnostics"]["warning"],
        "list_unselected_emphasis_1": r["syntax"]["link"],
        "list_unselected_emphasis_2": r["diagnostics"]["ok"],
        "list_unselected_emphasis_3": r["diagnostics"]["error"],
        "frame_selected_base": r["diagnostics"]["ok"],
        "frame_selected_background": flat_bg,
        "frame_selected_emphasis_0": r["diagnostics"]["warning"],
        "frame_selected_emphasis_1": r["syntax"]["link"],
        "frame_selected_emphasis_2": r["diagnostics"]["error"],
        "frame_selected_emphasis_3": flat_bg,
        "frame_unselected_base": r["ui"]["border"],
        "frame_unselected_background": flat_bg,
        "frame_unselected_emphasis_0": r["ui"]["fgMuted"],
        "frame_unselected_emphasis_1": r["ui"]["border"],
        "frame_unselected_emphasis_2": flat_bg,
        "frame_unselected_emphasis_3": flat_bg,
        "frame_highlight_base": r["diagnostics"]["warning"],
        "frame_highlight_background": flat_bg,
        "frame_highlight_emphasis_0": r["diagnostics"]["error"],
        "frame_highlight_emphasis_1": r["diagnostics"]["warning"],
        "frame_highlight_emphasis_2": r["diagnostics"]["warning"],
        "frame_highlight_emphasis_3": r["diagnostics"]["warning"],
        "exit_code_success_base": r["diagnostics"]["ok"],
        "exit_code_success_background": flat_bg,
        "exit_code_success_emphasis_0": r["syntax"]["link"],
        "exit_code_success_emphasis_1": r["ui"]["bgAlt"],
        "exit_code_success_emphasis_2": r["diagnostics"]["error"],
        "exit_code_success_emphasis_3": selected_bg,
        "exit_code_error_base": r["diagnostics"]["error"],
        "exit_code_error_background": flat_bg,
        "exit_code_error_emphasis_0": r["diagnostics"]["warning"],
        "exit_code_error_emphasis_1": flat_bg,
        "exit_code_error_emphasis_2": flat_bg,
        "exit_code_error_emphasis_3": flat_bg,
        **{f"player_{idx}": color for idx, color in enumerate(players, start=1)},
    }


def dedupe_zellij_player_attrs(attrs: dict[str, str], ctx: dict[str, Any]) -> dict[str, str]:
    r = ctx["roles"]
    p = ctx["palette"]
    light = ctx["meta"]["variant"] == "light"
    bg = r["ui"]["bg"]
    fg = r["ui"]["fg"]
    alternates = [
        r["ansi"]["brightRed"],
        r["ansi"]["brightBlue"],
        r["ansi"]["brightGreen"],
        r["ansi"]["brightYellow"],
        r["ansi"]["brightMagenta"],
        r["ansi"]["brightCyan"],
        r["diagnostics"]["error"],
        r["diagnostics"]["warning"],
        r["diagnostics"]["hint"],
        r["diagnostics"]["ok"],
        r["syntax"]["tag"],
        r["syntax"]["type"],
        p["orange"],
        p["pink"],
    ]
    used: list[str] = []
    out: dict[str, str] = {}
    for player, color in attrs.items():
        candidate_pool = [
            color,
            mix(color, fg, 0.12 if light else 0.08),
            mix(color, fg, 0.24 if light else 0.16),
            sharpen(color, bg, light=light),
            mix(color, bg, 0.18 if light else 0.1),
            *alternates,
        ]
        chosen = color
        for candidate in candidate_pool:
            adjusted = candidate
            if color_distance(adjusted, bg) < 88:
                adjusted = sharpen(adjusted, bg, light=light)
            if any(color_distance(adjusted, existing) < 52 for existing in used):
                continue
            chosen = adjusted
            break
        out[player] = chosen
        used.append(chosen)
    return out


def apply_zellij_override_sections(sections: list[dict[str, Any]], override: dict[str, Any] | None, ctx: dict[str, Any]) -> list[dict[str, Any]]:
    component_map = {section["component"]: section for section in sections}

    if override:
        for component, attrs in override.get("components", {}).items():
            section = component_map.get(component)
            if section is None:
                continue
            for attr, value in attrs.items():
                section["attrs"][attr] = resolve_context_value(value, ctx)

        players = override.get("players", {})
        if players:
            section = component_map.get("multiplayer_user_colors")
            if section is not None:
                for player, value in players.items():
                    section["attrs"][player] = resolve_context_value(value, ctx)

    player_section = component_map.get("multiplayer_user_colors")
    if player_section is not None:
        player_section["attrs"] = dedupe_zellij_player_attrs(player_section["attrs"], ctx)

    return sections


def zellij_theme_kdl(ctx: dict[str, Any], root: Path) -> str:
    template = load_zellij_template(root)
    slots = build_zellij_slots(ctx)
    theme_id = ctx["meta"]["id"]
    override = load_zellij_override(root, theme_id)
    sections = [
        {
            "component": section["component"],
            "attrs": {key: render_template_value(value, slots) for key, value in section["attrs"].items()},
        }
        for section in template["sections"]
    ]
    sections = apply_zellij_override_sections(sections, override, ctx)
    lines = [
        f"// Auto-generated from themes/core/{theme_id}.yaml",
        f"// Template: {template['name']} v{template['version']}",
        "themes {",
        f"    {theme_id} {{",
    ]
    for section in sections:
        lines.append(f"        {section['component']} {{")
        for attr, value in section["attrs"].items():
            lines.append(f"            {attr} {zellij_rgb(value)}")
        lines.append("        }")
    lines.extend(["    }", "}"])
    return "\n".join(lines) + "\n"


def render_json(data: dict[str, Any]) -> str:
    return json.dumps(data, indent=2, ensure_ascii=False) + "\n"


def write_text(path: Path, data: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(data)


def generate_theme(theme_path: Path, template: dict[str, Any], root: Path) -> dict[Path, str]:
    theme = load_and_validate_theme(theme_path, template)
    ctx = theme_context(theme)
    theme_id = ctx["meta"]["id"]

    ghostty_path = root / "exports" / "ghostty" / f"{theme_id}.conf"
    k9s_path = root / "exports" / "k9s" / f"{theme_id}.yaml"
    zed_path = root / "exports" / "zed" / f"{theme_id}.json"
    nvim_path = root / "exports" / "nvim" / f"{theme_id}.lua"
    starship_path = root / "exports" / "starship" / f"{theme_id}.toml"
    zellij_path = root / "exports" / "zellij" / f"{theme_id}.kdl"
    tmux_path = root / "exports" / "tmux" / f"{theme_id}.conf"
    pi_path = root / "exports" / "pi" / f"{theme_id}.json"
    herdr_path = root / "exports" / "herdr" / f"{theme_id}.toml"

    return {
        ghostty_path: ghostty_theme_conf(ctx, root),
        k9s_path: render_yaml(k9s_theme_doc(ctx, root)),
        zed_path: render_json(zed_theme_doc(ctx, root)),
        nvim_path: nvim_lua(ctx, root),
        starship_path: starship_theme_toml(ctx, root),
        zellij_path: zellij_theme_kdl(ctx, root),
        tmux_path: tmux_theme_conf(ctx, root),
        pi_path: render_json(pi_theme_doc(ctx, root)),
        herdr_path: herdr_theme_toml(ctx, root),
    }


def discover_default_targets(root: Path) -> list[Path]:
    return sorted((root / "core").glob("*.yaml"))


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate theme exports from canonical YAML schema.")
    parser.add_argument("paths", nargs="*", help="Theme YAML files to generate. Defaults to themes/core/*.yaml")
    parser.add_argument("--template", default=None, help="Path to template YAML. Defaults to themes/TEMPLATE.yaml")
    args = parser.parse_args()

    root = Path(__file__).resolve().parent
    template_path = Path(args.template).resolve() if args.template else root / "TEMPLATE.yaml"

    try:
        template = load_yaml(template_path)
    except ParseError as e:
        print(f"error: failed to parse template: {e}", file=sys.stderr)
        return 2

    targets = [Path(p).resolve() for p in args.paths] if args.paths else [p.resolve() for p in discover_default_targets(root)]
    if not targets:
        print("No theme files found to generate.", file=sys.stderr)
        return 2

    generated: list[tuple[Path, str]] = []
    for target in targets:
        try:
            generated.extend(generate_theme(target, template, root).items())
        except Exception as e:  # noqa: BLE001
            print(f"error: failed to generate {target}: {e}", file=sys.stderr)
            return 1

    for path, content in generated:
        try:
            write_text(path, content)
        except OSError as e:
            print(f"error: failed to write {path}: {e}", file=sys.stderr)
            return 1

    for path, _ in generated:
        print(path.relative_to(root.parent))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
