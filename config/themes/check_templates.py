#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

from validate import Validator, load_yaml
from validate_overrides import validate_ghostty_override, validate_starship_override, validate_zed_override

ROOT = Path(__file__).resolve().parent


def load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def load_yaml_doc(path: Path) -> dict[str, Any]:
    return load_yaml(path)


def expect(condition: bool, message: str, errors: list[str]) -> None:
    if not condition:
        errors.append(message)


def require_keys(obj: dict[str, Any], keys: list[str], path: str, errors: list[str]) -> None:
    for key in keys:
        expect(key in obj, f"{path}: missing key {key!r}", errors)


def check_source_list(path: str, sources: Any, errors: list[str]) -> None:
    expect(isinstance(sources, list) and len(sources) > 0, f"{path}: sources must be a non-empty list", errors)
    if not isinstance(sources, list):
        return

    for idx, source in enumerate(sources):
        item_path = f"{path}.sources[{idx}]"
        expect(isinstance(source, dict), f"{item_path}: source must be an object", errors)
        if not isinstance(source, dict):
            continue
        require_keys(source, ["kind", "ref"], item_path, errors)
        expect(any(key in source for key in ("url", "path")), f"{item_path}: source must include either 'url' or 'path'", errors)
        expect(isinstance(source.get("kind"), str), f"{item_path}.kind must be a string", errors)
        expect(isinstance(source.get("ref"), str), f"{item_path}.ref must be a string", errors)
        if "url" in source:
            expect(isinstance(source["url"], str) and source["url"], f"{item_path}.url must be a non-empty string", errors)
        if "path" in source:
            expect(isinstance(source["path"], str) and source["path"], f"{item_path}.path must be a non-empty string", errors)


def check_section_names(path: str, sections: list[dict[str, Any]], errors: list[str]) -> None:
    seen: set[str] = set()
    for idx, section in enumerate(sections):
        item_path = f"{path}[{idx}]"
        expect(isinstance(section, dict), f"{item_path}: section must be an object", errors)
        if not isinstance(section, dict):
            continue
        expect(isinstance(section.get("name"), str) and section["name"], f"{item_path}.name must be a non-empty string", errors)
        name = section.get("name")
        if isinstance(name, str):
            expect(name not in seen, f"{path}: duplicate section name {name!r}", errors)
            seen.add(name)


def check_nvim_sections(path: str, sections: Any, errors: list[str]) -> None:
    expect(isinstance(sections, list) and len(sections) > 0, f"{path}: sections must be a non-empty list", errors)
    if not isinstance(sections, list):
        return
    check_section_names(path, sections, errors)
    for idx, section in enumerate(sections):
        if not isinstance(section, dict):
            continue
        groups = section.get("groups")
        item_path = f"{path}[{idx}]"
        expect(isinstance(groups, list) and len(groups) > 0, f"{item_path}.groups must be a non-empty list", errors)
        if not isinstance(groups, list):
            continue
        seen_groups: set[str] = set()
        for gidx, group in enumerate(groups):
            group_path = f"{item_path}.groups[{gidx}]"
            expect(isinstance(group, dict), f"{group_path}: group spec must be an object", errors)
            if not isinstance(group, dict):
                continue
            require_keys(group, ["group", "attrs"], group_path, errors)
            expect(isinstance(group.get("group"), str) and group["group"], f"{group_path}.group must be a non-empty string", errors)
            expect(isinstance(group.get("attrs"), dict), f"{group_path}.attrs must be an object", errors)
            name = group.get("group")
            if isinstance(name, str):
                expect(name not in seen_groups, f"{item_path}: duplicate group name {name!r}", errors)
                seen_groups.add(name)


def check_zed_style_sections(path: str, sections: Any, errors: list[str]) -> set[str]:
    keys: set[str] = set()
    expect(isinstance(sections, list) and len(sections) > 0, f"{path}: style_sections must be a non-empty list", errors)
    if not isinstance(sections, list):
        return keys
    check_section_names(path, sections, errors)
    for idx, section in enumerate(sections):
        if not isinstance(section, dict):
            continue
        entries = section.get("entries")
        item_path = f"{path}[{idx}]"
        expect(isinstance(entries, list) and len(entries) > 0, f"{item_path}.entries must be a non-empty list", errors)
        if not isinstance(entries, list):
            continue
        for eidx, entry in enumerate(entries):
            entry_path = f"{item_path}.entries[{eidx}]"
            expect(isinstance(entry, dict), f"{entry_path}: entry must be an object", errors)
            if not isinstance(entry, dict):
                continue
            require_keys(entry, ["key", "value"], entry_path, errors)
            expect(isinstance(entry.get("key"), str) and entry["key"], f"{entry_path}.key must be a non-empty string", errors)
            key = entry.get("key")
            if isinstance(key, str):
                expect(key not in keys, f"{path}: duplicate style key {key!r}", errors)
                keys.add(key)
    return keys


def check_zed_syntax_sections(path: str, sections: Any, errors: list[str]) -> set[str]:
    keys: set[str] = set()
    expect(isinstance(sections, list) and len(sections) > 0, f"{path}: syntax_sections must be a non-empty list", errors)
    if not isinstance(sections, list):
        return keys
    check_section_names(path, sections, errors)
    for idx, section in enumerate(sections):
        if not isinstance(section, dict):
            continue
        entries = section.get("entries")
        item_path = f"{path}[{idx}]"
        expect(isinstance(entries, list) and len(entries) > 0, f"{item_path}.entries must be a non-empty list", errors)
        if not isinstance(entries, list):
            continue
        for eidx, entry in enumerate(entries):
            entry_path = f"{item_path}.entries[{eidx}]"
            expect(isinstance(entry, dict), f"{entry_path}: entry must be an object", errors)
            if not isinstance(entry, dict):
                continue
            require_keys(entry, ["key", "value"], entry_path, errors)
            expect(isinstance(entry.get("key"), str) and entry["key"], f"{entry_path}.key must be a non-empty string", errors)
            expect(isinstance(entry.get("value"), dict), f"{entry_path}.value must be an object", errors)
            key = entry.get("key")
            if isinstance(key, str):
                expect(key not in keys, f"{path}: duplicate syntax key {key!r}", errors)
                keys.add(key)
    return keys


def check_common(path: Path, doc: dict[str, Any], errors: list[str]) -> None:
    path_str = str(path)
    require_keys(doc, ["name", "version", "description", "sources"], path_str, errors)
    expect(isinstance(doc.get("name"), str) and doc["name"], f"{path_str}.name must be a non-empty string", errors)
    expect(isinstance(doc.get("version"), (str, int)), f"{path_str}.version must be a string or integer", errors)
    expect(isinstance(doc.get("description"), str) and doc["description"], f"{path_str}.description must be a non-empty string", errors)
    check_source_list(path_str, doc.get("sources"), errors)


def check_document_template(path: str, document: Any, errors: list[str]) -> None:
    expect(isinstance(document, dict) and len(document) > 0, f"{path}.document must be a non-empty object", errors)


def check_nvim(path: Path, doc: dict[str, Any], errors: list[str]) -> None:
    check_common(path, doc, errors)
    path_str = str(path)
    contract = doc.get("highlight_schema")
    expect(isinstance(contract, dict), f"{path_str}.highlight_schema must be an object", errors)
    if isinstance(contract, dict):
        require_keys(contract, ["api", "allowed_keys", "token_prefix", "token_note"], f"{path_str}.highlight_schema", errors)
        expect(contract.get("api") == "nvim_set_hl", f"{path_str}.highlight_schema.api must be 'nvim_set_hl'", errors)
        expect(contract.get("token_prefix") == "$", f"{path_str}.highlight_schema.token_prefix must be '$'", errors)
        expect(isinstance(contract.get("allowed_keys"), list) and len(contract["allowed_keys"]) > 0, f"{path_str}.highlight_schema.allowed_keys must be a non-empty list", errors)
    check_nvim_sections(f"{path_str}.sections", doc.get("sections"), errors)


def check_zed(path: Path, doc: dict[str, Any], errors: list[str]) -> None:
    check_common(path, doc, errors)
    path_str = str(path)
    contract = doc.get("theme_family_schema")
    expect(isinstance(contract, dict), f"{path_str}.theme_family_schema must be an object", errors)
    if isinstance(contract, dict):
        require_keys(contract, ["schema_url", "token_prefix", "style_key_source", "syntax_value_schema"], f"{path_str}.theme_family_schema", errors)
        expect(contract.get("token_prefix") == "$", f"{path_str}.theme_family_schema.token_prefix must be '$'", errors)
        expect(contract.get("style_key_source") == "official-json-schema", f"{path_str}.theme_family_schema.style_key_source must be 'official-json-schema'", errors)
    style_keys = check_zed_style_sections(f"{path_str}.style_sections", doc.get("style_sections"), errors)
    syntax_keys = check_zed_syntax_sections(f"{path_str}.syntax_sections", doc.get("syntax_sections"), errors)
    players = doc.get("players")
    expect(isinstance(players, list) and len(players) > 0, f"{path_str}.players must be a non-empty list", errors)
    if isinstance(players, list):
        for idx, player in enumerate(players):
            item_path = f"{path_str}.players[{idx}]"
            expect(isinstance(player, dict), f"{item_path}: player entry must be an object", errors)
            if not isinstance(player, dict):
                continue
            require_keys(player, ["cursor", "background", "selection"], item_path, errors)
    declared_style_keys = doc.get("style_keys")
    expect(isinstance(declared_style_keys, list) and len(declared_style_keys) > 0, f"{path_str}.style_keys must be a non-empty list", errors)
    if isinstance(declared_style_keys, list):
        actual = style_keys | {"players", "syntax"}
        declared = set(declared_style_keys)
        expect(declared == actual, f"{path_str}.style_keys must exactly match style section keys plus players/syntax", errors)
    expect(len(syntax_keys) > 0, f"{path_str}: syntax_sections must declare at least one syntax key", errors)


def check_nvim_plugin_template(path: Path, doc: dict[str, Any], errors: list[str]) -> None:
    check_common(path, doc, errors)
    check_nvim_sections(f"{path}.sections", doc.get("sections"), errors)


def check_ghostty_sections(path: str, sections: Any, errors: list[str]) -> None:
    expect(isinstance(sections, list) and len(sections) > 0, f"{path}: sections must be a non-empty list", errors)
    if not isinstance(sections, list):
        return
    check_section_names(path, sections, errors)
    for idx, section in enumerate(sections):
        item_path = f"{path}[{idx}]"
        expect(isinstance(section, dict), f"{item_path}: section must be an object", errors)
        if not isinstance(section, dict):
            continue
        require_keys(section, ["name", "entries"], item_path, errors)
        entries = section.get("entries")
        expect(isinstance(entries, list) and len(entries) > 0, f"{item_path}.entries must be a non-empty list", errors)
        if not isinstance(entries, list):
            continue
        seen_keys: set[tuple[str, Any]] = set()
        for eidx, entry in enumerate(entries):
            entry_path = f"{item_path}.entries[{eidx}]"
            expect(isinstance(entry, dict), f"{entry_path}: entry must be an object", errors)
            if not isinstance(entry, dict):
                continue
            require_keys(entry, ["key", "value"], entry_path, errors)
            key = entry.get("key")
            value = entry.get("value")
            expect(isinstance(key, str) and key, f"{entry_path}.key must be a non-empty string", errors)
            expect(isinstance(value, str) and value, f"{entry_path}.value must be a non-empty string", errors)
            if isinstance(key, str) and isinstance(value, str):
                fingerprint = (key, value)
                expect(fingerprint not in seen_keys, f"{item_path}: duplicate entry {(key, value)!r}", errors)
                seen_keys.add(fingerprint)


def check_ghostty(path: Path, doc: dict[str, Any], errors: list[str]) -> None:
    check_common(path, doc, errors)
    path_str = str(path)
    contract = doc.get("config_schema")
    expect(isinstance(contract, dict), f"{path_str}.config_schema must be an object", errors)
    if isinstance(contract, dict):
        require_keys(contract, ["token_prefix", "value_type"], f"{path_str}.config_schema", errors)
        expect(contract.get("token_prefix") == "$", f"{path_str}.config_schema.token_prefix must be '$'", errors)
        expect(contract.get("value_type") == "hex-or-palette-entry-or-opacity", f"{path_str}.config_schema.value_type must be 'hex-or-palette-entry-or-opacity'", errors)
    check_ghostty_sections(f"{path_str}.sections", doc.get("sections"), errors)


def check_starship_sections(path: str, sections: Any, errors: list[str]) -> None:
    expect(isinstance(sections, list) and len(sections) > 0, f"{path}: sections must be a non-empty list", errors)
    if not isinstance(sections, list):
        return
    check_section_names(path, sections, errors)
    seen_tables: set[str] = set()
    for idx, section in enumerate(sections):
        item_path = f"{path}[{idx}]"
        expect(isinstance(section, dict), f"{item_path}: section must be an object", errors)
        if not isinstance(section, dict):
            continue
        require_keys(section, ["name", "table", "entries"], item_path, errors)
        table = section.get("table")
        expect(isinstance(table, str) and table, f"{item_path}.table must be a non-empty string", errors)
        if isinstance(table, str):
            expect(table not in seen_tables, f"{path}: duplicate table {table!r}", errors)
            seen_tables.add(table)
        entries = section.get("entries")
        expect(isinstance(entries, list) and len(entries) > 0, f"{item_path}.entries must be a non-empty list", errors)
        if not isinstance(entries, list):
            continue
        seen_keys: set[str] = set()
        for eidx, entry in enumerate(entries):
            entry_path = f"{item_path}.entries[{eidx}]"
            expect(isinstance(entry, dict), f"{entry_path}: entry must be an object", errors)
            if not isinstance(entry, dict):
                continue
            require_keys(entry, ["key", "value"], entry_path, errors)
            key = entry.get("key")
            expect(isinstance(key, str) and key, f"{entry_path}.key must be a non-empty string", errors)
            if isinstance(key, str):
                expect(key not in seen_keys, f"{item_path}: duplicate key {key!r}", errors)
                seen_keys.add(key)


def check_starship(path: Path, doc: dict[str, Any], errors: list[str]) -> None:
    check_common(path, doc, errors)
    path_str = str(path)
    contract = doc.get("toml_schema")
    expect(isinstance(contract, dict), f"{path_str}.toml_schema must be an object", errors)
    if isinstance(contract, dict):
        require_keys(contract, ["format_key", "palette_table_prefix", "value_types"], f"{path_str}.toml_schema", errors)
        expect(contract.get("format_key") == "format", f"{path_str}.toml_schema.format_key must be 'format'", errors)
        expect(contract.get("palette_table_prefix") == "palettes", f"{path_str}.toml_schema.palette_table_prefix must be 'palettes'", errors)
        expect(isinstance(contract.get("value_types"), list) and len(contract["value_types"]) > 0, f"{path_str}.toml_schema.value_types must be a non-empty list", errors)
    check_starship_sections(f"{path_str}.sections", doc.get("sections"), errors)


def check_zellij_sections(path: str, sections: Any, errors: list[str]) -> None:
    expect(isinstance(sections, list) and len(sections) > 0, f"{path}: sections must be a non-empty list", errors)
    if not isinstance(sections, list):
        return
    seen_components: set[str] = set()
    for idx, section in enumerate(sections):
        item_path = f"{path}[{idx}]"
        expect(isinstance(section, dict), f"{item_path}: section must be an object", errors)
        if not isinstance(section, dict):
            continue
        require_keys(section, ["component", "attrs"], item_path, errors)
        component = section.get("component")
        expect(isinstance(component, str) and component, f"{item_path}.component must be a non-empty string", errors)
        if isinstance(component, str):
            expect(component not in seen_components, f"{path}: duplicate component {component!r}", errors)
            seen_components.add(component)
        attrs = section.get("attrs")
        expect(isinstance(attrs, dict) and len(attrs) > 0, f"{item_path}.attrs must be a non-empty object", errors)


def check_zellij(path: Path, doc: dict[str, Any], errors: list[str]) -> None:
    check_common(path, doc, errors)
    path_str = str(path)
    contract = doc.get("theme_schema")
    expect(isinstance(contract, dict), f"{path_str}.theme_schema must be an object", errors)
    if isinstance(contract, dict):
        require_keys(contract, ["root_node", "token_prefix", "value_type"], f"{path_str}.theme_schema", errors)
        expect(contract.get("root_node") == "themes", f"{path_str}.theme_schema.root_node must be 'themes'", errors)
        expect(contract.get("token_prefix") == "$", f"{path_str}.theme_schema.token_prefix must be '$'", errors)
        expect(contract.get("value_type") == "rgb-triplet", f"{path_str}.theme_schema.value_type must be 'rgb-triplet'", errors)
    check_zellij_sections(f"{path_str}.sections", doc.get("sections"), errors)


def check_tmux_sections(path: str, sections: object, errors: list[str]) -> None:
    expect(isinstance(sections, list) and len(sections) > 0, f"{path}: sections must be a non-empty list", errors)
    if not isinstance(sections, list):
        return
    check_section_names(path, sections, errors)
    seen_options: set[str] = set()
    for idx, section in enumerate(sections):
        item_path = f"{path}[{idx}]"
        expect(isinstance(section, dict), f"{item_path}: section must be an object", errors)
        if not isinstance(section, dict):
            continue
        require_keys(section, ["name", "entries"], item_path, errors)
        entries = section.get("entries")
        expect(isinstance(entries, list) and len(entries) > 0, f"{item_path}.entries must be a non-empty list", errors)
        if not isinstance(entries, list):
            continue
        for eidx, entry in enumerate(entries):
            entry_path = f"{item_path}.entries[{eidx}]"
            expect(isinstance(entry, dict), f"{entry_path}: entry must be an object", errors)
            if not isinstance(entry, dict):
                continue
            require_keys(entry, ["scope", "option", "value"], entry_path, errors)
            scope = entry.get("scope")
            option = entry.get("option")
            value = entry.get("value")
            expect(scope in {"set", "setw"}, f"{entry_path}.scope must be 'set' or 'setw'", errors)
            expect(isinstance(option, str) and option, f"{entry_path}.option must be a non-empty string", errors)
            expect(isinstance(value, str) and value, f"{entry_path}.value must be a non-empty string", errors)
            if isinstance(scope, str) and isinstance(option, str):
                fingerprint = f"{scope}:{option}"
                expect(fingerprint not in seen_options, f"{path}: duplicate tmux option {fingerprint!r}", errors)
                seen_options.add(fingerprint)


def check_tmux(path: Path, doc: dict[str, Any], errors: list[str]) -> None:
    check_common(path, doc, errors)
    path_str = str(path)
    contract = doc.get("config_schema")
    expect(isinstance(contract, dict), f"{path_str}.config_schema must be an object", errors)
    if isinstance(contract, dict):
        require_keys(contract, ["token_prefix", "value_type"], f"{path_str}.config_schema", errors)
        expect(contract.get("token_prefix") == "$", f"{path_str}.config_schema.token_prefix must be '$'", errors)
        expect(contract.get("value_type") == "tmux-style-or-format", f"{path_str}.config_schema.value_type must be 'tmux-style-or-format'", errors)
    check_tmux_sections(f"{path_str}.sections", doc.get("sections"), errors)


def check_k9s(path: Path, doc: dict[str, Any], errors: list[str]) -> None:
    check_common(path, doc, errors)
    path_str = str(path)
    contract = doc.get("skin_schema")
    expect(isinstance(contract, dict), f"{path_str}.skin_schema must be an object", errors)
    if isinstance(contract, dict):
        require_keys(contract, ["format", "root_key", "token_prefix", "value_types"], f"{path_str}.skin_schema", errors)
        expect(contract.get("format") == "yaml", f"{path_str}.skin_schema.format must be 'yaml'", errors)
        expect(contract.get("root_key") == "k9s", f"{path_str}.skin_schema.root_key must be 'k9s'", errors)
        expect(contract.get("token_prefix") == "$", f"{path_str}.skin_schema.token_prefix must be '$'", errors)
        expect(isinstance(contract.get("value_types"), list) and len(contract["value_types"]) > 0, f"{path_str}.skin_schema.value_types must be a non-empty list", errors)
    check_document_template(path_str, doc.get("document"), errors)


def check_herdr(path: Path, doc: dict[str, Any], errors: list[str]) -> None:
    check_common(path, doc, errors)
    path_str = str(path)
    contract = doc.get("config_schema", {})
    expect(contract == {"format": "toml", "token_prefix": "$", "value_type": "hex-color"},
           f"{path_str}.config_schema must describe TOML hex colors with '$' tokens", errors)
    sections = doc.get("sections")
    if not isinstance(sections, list) or len(sections) != 2:
        errors.append(f"{path_str}.sections must contain theme and theme.custom tables")
        return
    check_section_names(path_str, sections, errors)
    color_keys = {
        "accent", "panel_bg", "sidebar_bg", "active_row_bg", "selection_bg",
        "surface0", "surface1", "surface_dim", "overlay0", "overlay1", "text",
        "subtext0", "mauve", "green", "yellow", "red", "blue", "teal", "peach",
    }
    for section, table, keys in zip(sections, ("theme", "theme.custom"), ({"name", "auto_switch"}, color_keys)):
        if not isinstance(section, dict):
            errors.append(f"{path_str}: section must be an object")
            continue
        expect(section.get("table") == table, f"{path_str}: expected table {table}", errors)
        entries = section.get("entries")
        if not isinstance(entries, list) or not all(isinstance(entry, dict) for entry in entries):
            errors.append(f"{path_str}.{table}: entries must be objects")
            continue
        actual_keys = [entry.get("key") for entry in entries]
        expect(len(actual_keys) == len(keys) and all(key in actual_keys for key in keys),
               f"{path_str}.{table}: keys must match the supported Herdr fields", errors)
        for entry in entries:
            value = entry.get("value")
            if table == "theme.custom":
                expect(isinstance(value, str) and value.startswith("$"),
                       f"{path_str}.{table}.{entry.get('key')}: expected a color slot", errors)
            elif entry.get("key") == "name":
                expect(value == "terminal", f"{path_str}: base theme must be terminal", errors)
            elif entry.get("key") == "auto_switch":
                expect(value is False, f"{path_str}: automatic theme switching must be disabled", errors)


def check_pi(path: Path, doc: dict[str, Any], errors: list[str]) -> None:
    check_common(path, doc, errors)
    path_str = str(path)
    contract = doc.get("theme_schema")
    expect(isinstance(contract, dict), f"{path_str}.theme_schema must be an object", errors)
    if isinstance(contract, dict):
        require_keys(contract, ["schema_url", "token_prefix", "value_type"], f"{path_str}.theme_schema", errors)
        expect(contract.get("token_prefix") == "$", f"{path_str}.theme_schema.token_prefix must be '$'", errors)
        expect(contract.get("value_type") == "hex-color", f"{path_str}.theme_schema.value_type must be 'hex-color'", errors)

    document = doc.get("document")
    check_document_template(path_str, document, errors)
    if not isinstance(document, dict):
        return
    require_keys(document, ["$schema", "name", "colors", "export"], f"{path_str}.document", errors)
    colors = document.get("colors")
    expect(isinstance(colors, dict), f"{path_str}.document.colors must be an object", errors)
    if not isinstance(colors, dict):
        return
    required_colors = {
        "accent",
        "border",
        "borderAccent",
        "borderMuted",
        "success",
        "error",
        "warning",
        "muted",
        "dim",
        "text",
        "thinkingText",
        "selectedBg",
        "scrollbarTrack",
        "scrollbarThumb",
        "searchMatchBg",
        "searchMatchText",
        "userMessageBg",
        "userMessageText",
        "customMessageBg",
        "customMessageText",
        "customMessageLabel",
        "toolPendingBg",
        "toolSuccessBg",
        "toolErrorBg",
        "toolTitle",
        "toolOutput",
        "mdHeading",
        "mdLink",
        "mdLinkUrl",
        "mdCode",
        "mdCodeBlock",
        "mdCodeBlockBorder",
        "mdQuote",
        "mdQuoteBorder",
        "mdHr",
        "mdListBullet",
        "toolDiffAdded",
        "toolDiffRemoved",
        "toolDiffContext",
        "syntaxComment",
        "syntaxKeyword",
        "syntaxFunction",
        "syntaxVariable",
        "syntaxString",
        "syntaxNumber",
        "syntaxType",
        "syntaxOperator",
        "syntaxPunctuation",
        "thinkingOff",
        "thinkingMinimal",
        "thinkingLow",
        "thinkingMedium",
        "thinkingHigh",
        "thinkingXhigh",
        "thinkingMax",
        "bashMode",
    }
    expect(set(colors) == required_colors, f"{path_str}.document.colors must exactly match Pi's color tokens", errors)


def check_nvim_override_template(path: Path, doc: dict[str, Any], errors: list[str]) -> None:
    path_str = str(path)
    expected_top = ["version", "meta", "groups", "links"]
    expect(list(doc.keys()) == expected_top, f"{path_str}: top-level keys must exactly match {expected_top}", errors)

    meta = doc.get("meta")
    expect(isinstance(meta, dict), f"{path_str}.meta must be an object", errors)
    if isinstance(meta, dict):
        expected_meta = ["app", "theme", "variant"]
        expect(list(meta.keys()) == expected_meta, f"{path_str}.meta keys must exactly match {expected_meta}", errors)

    groups = doc.get("groups")
    expect(isinstance(groups, dict) and len(groups) > 0, f"{path_str}.groups must be a non-empty object", errors)

    links = doc.get("links")
    expect(isinstance(links, dict), f"{path_str}.links must be an object", errors)


def check_ghostty_override_template(path: Path, doc: dict[str, Any], errors: list[str]) -> None:
    path_str = str(path)
    expected_top = ["version", "meta", "slots"]
    expect(list(doc.keys()) == expected_top, f"{path_str}: top-level keys must exactly match {expected_top}", errors)

    meta = doc.get("meta")
    expect(isinstance(meta, dict), f"{path_str}.meta must be an object", errors)
    if isinstance(meta, dict):
        expected_meta = ["app", "theme", "variant"]
        expect(list(meta.keys()) == expected_meta, f"{path_str}.meta keys must exactly match {expected_meta}", errors)

    slots = doc.get("slots")
    expect(isinstance(slots, dict) and len(slots) > 0, f"{path_str}.slots must be a non-empty object", errors)


def check_ghostty_override(path: Path, doc: dict[str, Any], errors: list[str]) -> None:
    path_str = str(path)
    expected_top = ["version", "meta", "slots"]
    expect(list(doc.keys()) == expected_top, f"{path_str}: top-level keys must exactly match {expected_top}", errors)
    expect(doc.get("version") == 1, f"{path_str}.version must be 1", errors)

    meta = doc.get("meta")
    expect(isinstance(meta, dict), f"{path_str}.meta must be an object", errors)
    if isinstance(meta, dict):
        expected_meta = ["app", "theme", "variant"]
        expect(list(meta.keys()) == expected_meta, f"{path_str}.meta keys must exactly match {expected_meta}", errors)
        expect(meta.get("app") == "ghostty", f"{path_str}.meta.app must be 'ghostty'", errors)
        expect(isinstance(meta.get("theme"), str) and meta.get("theme"), f"{path_str}.meta.theme must be a non-empty string", errors)
        expect(meta.get("variant") in {"light", "dark"}, f"{path_str}.meta.variant must be 'light' or 'dark'", errors)

    slots = doc.get("slots")
    expect(isinstance(slots, dict) and len(slots) > 0, f"{path_str}.slots must be a non-empty object", errors)
    if isinstance(slots, dict):
        for slot, value in slots.items():
            expect(isinstance(slot, str) and slot, f"{path_str}.slots contains an invalid slot name", errors)
            expect(isinstance(value, str) and value, f"{path_str}.slots.{slot} must be a non-empty string", errors)


def check_nvim_override(path: Path, doc: dict[str, Any], errors: list[str]) -> None:
    path_str = str(path)
    expected_top = ["version", "meta", "groups", "links"]
    expect(list(doc.keys()) == expected_top, f"{path_str}: top-level keys must exactly match {expected_top}", errors)

    expect(doc.get("version") == 1, f"{path_str}.version must be 1", errors)

    meta = doc.get("meta")
    expect(isinstance(meta, dict), f"{path_str}.meta must be an object", errors)
    if isinstance(meta, dict):
        expected_meta = ["app", "theme", "variant"]
        expect(list(meta.keys()) == expected_meta, f"{path_str}.meta keys must exactly match {expected_meta}", errors)
        expect(meta.get("app") == "nvim", f"{path_str}.meta.app must be 'nvim'", errors)
        expect(isinstance(meta.get("theme"), str) and meta.get("theme"), f"{path_str}.meta.theme must be a non-empty string", errors)
        expect(meta.get("variant") in {"light", "dark"}, f"{path_str}.meta.variant must be 'light' or 'dark'", errors)

    groups = doc.get("groups")
    expect(isinstance(groups, dict) and len(groups) > 0, f"{path_str}.groups must be a non-empty object", errors)
    if isinstance(groups, dict):
        seen_groups: set[str] = set()
        for group, attrs in groups.items():
            expect(isinstance(group, str) and group, f"{path_str}.groups contains an invalid group name", errors)
            if isinstance(group, str):
                expect(group not in seen_groups, f"{path_str}.groups duplicate group name {group!r}", errors)
                seen_groups.add(group)
            expect(isinstance(attrs, dict) and len(attrs) > 0, f"{path_str}.groups.{group} must be a non-empty object", errors)

    links = doc.get("links")
    expect(isinstance(links, dict), f"{path_str}.links must be an object", errors)
    if isinstance(links, dict):
        for group, target in links.items():
            expect(isinstance(group, str) and group, f"{path_str}.links contains an invalid group name", errors)
            expect(isinstance(target, str) and target, f"{path_str}.links.{group} must be a non-empty string", errors)


def check_zellij_override_template(path: Path, doc: dict[str, Any], errors: list[str]) -> None:
    path_str = str(path)
    expected_top = ["version", "meta", "components", "players"]
    expect(list(doc.keys()) == expected_top, f"{path_str}: top-level keys must exactly match {expected_top}", errors)

    meta = doc.get("meta")
    expect(isinstance(meta, dict), f"{path_str}.meta must be an object", errors)
    if isinstance(meta, dict):
        expected_meta = ["app", "theme", "variant"]
        expect(list(meta.keys()) == expected_meta, f"{path_str}.meta keys must exactly match {expected_meta}", errors)

    components = doc.get("components")
    expect(isinstance(components, dict), f"{path_str}.components must be an object", errors)

    players = doc.get("players")
    expect(isinstance(players, dict), f"{path_str}.players must be an object", errors)


def check_zellij_override(path: Path, doc: dict[str, Any], errors: list[str]) -> None:
    path_str = str(path)
    expected_top = ["version", "meta", "components", "players"]
    expect(list(doc.keys()) == expected_top, f"{path_str}: top-level keys must exactly match {expected_top}", errors)
    expect(doc.get("version") == 1, f"{path_str}.version must be 1", errors)

    meta = doc.get("meta")
    expect(isinstance(meta, dict), f"{path_str}.meta must be an object", errors)
    if isinstance(meta, dict):
        expected_meta = ["app", "theme", "variant"]
        expect(list(meta.keys()) == expected_meta, f"{path_str}.meta keys must exactly match {expected_meta}", errors)
        expect(meta.get("app") == "zellij", f"{path_str}.meta.app must be 'zellij'", errors)
        expect(isinstance(meta.get("theme"), str) and meta.get("theme"), f"{path_str}.meta.theme must be a non-empty string", errors)
        expect(meta.get("variant") in {"light", "dark"}, f"{path_str}.meta.variant must be 'light' or 'dark'", errors)

    components = doc.get("components")
    players = doc.get("players")
    expect(isinstance(components, dict), f"{path_str}.components must be an object", errors)
    expect(isinstance(players, dict), f"{path_str}.players must be an object", errors)
    if isinstance(components, dict):
        for component, attrs in components.items():
            expect(isinstance(component, str) and component, f"{path_str}.components contains an invalid component name", errors)
            expect(isinstance(attrs, dict) and len(attrs) > 0, f"{path_str}.components.{component} must be a non-empty object", errors)
    if isinstance(players, dict):
        for player, value in players.items():
            expect(isinstance(player, str) and player, f"{path_str}.players contains an invalid player name", errors)
            expect(isinstance(value, str) and value, f"{path_str}.players.{player} must be a non-empty string", errors)
    if isinstance(components, dict) and isinstance(players, dict):
        expect(bool(components) or bool(players), f"{path_str} must contain at least one component or player override", errors)


JSON_CHECKS = {
    ROOT / "templates" / "k9s" / "official-template.json": check_k9s,
    ROOT / "templates" / "ghostty" / "official-template.json": check_ghostty,
    ROOT / "templates" / "nvim" / "official-template.json": check_nvim,
    ROOT / "templates" / "zed" / "official-template.json": check_zed,
    ROOT / "templates" / "nvim" / "plugins.json": check_nvim_plugin_template,
    ROOT / "templates" / "starship" / "official-template.json": check_starship,
    ROOT / "templates" / "zellij" / "official-template.json": check_zellij,
    ROOT / "templates" / "tmux" / "official-template.json": check_tmux,
    ROOT / "templates" / "pi" / "official-template.json": check_pi,
    ROOT / "templates" / "herdr" / "official-template.json": check_herdr,
}

YAML_CHECKS = {
    ROOT / "overrides" / "TEMPLATE.yaml": check_nvim_override_template,
    ROOT / "overrides" / "ghostty" / "TEMPLATE.yaml": check_ghostty_override_template,
    ROOT / "overrides" / "zellij" / "TEMPLATE.yaml": check_zellij_override_template,
}


def main() -> int:
    errors: list[str] = []
    for path, checker in JSON_CHECKS.items():
        checker(path.relative_to(ROOT), load(path), errors)

    for path, checker in YAML_CHECKS.items():
        checker(path.relative_to(ROOT), load_yaml_doc(path), errors)

    starship_overrides = sorted((ROOT / "overrides" / "starship").glob("*.yaml"))
    for path in starship_overrides:
        validator = Validator()
        validate_starship_override(validator, path)
        errors.extend(validator.errors)

    zed_overrides = sorted((ROOT / "overrides" / "zed").glob("*.yaml"))
    for path in zed_overrides:
        validator = Validator()
        validate_zed_override(validator, path)
        errors.extend(validator.errors)

    for path in sorted((ROOT / "overrides" / "ghostty").glob("*.yaml")):
        if path.name == "TEMPLATE.yaml":
            continue
        check_ghostty_override(path.relative_to(ROOT), load_yaml_doc(path), errors)
        validator = Validator()
        validate_ghostty_override(validator, path)
        errors.extend(validator.errors)
    for path in sorted((ROOT / "overrides" / "nvim").glob("*.yaml")):
        check_nvim_override(path.relative_to(ROOT), load_yaml_doc(path), errors)
    for path in sorted((ROOT / "overrides" / "zellij").glob("*.yaml")):
        if path.name == "TEMPLATE.yaml":
            continue
        check_zellij_override(path.relative_to(ROOT), load_yaml_doc(path), errors)

    if errors:
        print("Template consistency check failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    total = (
        len(JSON_CHECKS)
        + len(YAML_CHECKS)
        + len(starship_overrides)
        + len(zed_overrides)
        + len([p for p in (ROOT / "overrides" / "ghostty").glob("*.yaml") if p.name != "TEMPLATE.yaml"])
        + len(list((ROOT / "overrides" / "nvim").glob("*.yaml")))
        + len([p for p in (ROOT / "overrides" / "zellij").glob("*.yaml") if p.name != "TEMPLATE.yaml"])
    )
    print(f"Validated {total} template/override file(s) successfully.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
