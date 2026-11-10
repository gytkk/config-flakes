#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path
from typing import Any


HEX_RE = re.compile(r"^#[0-9a-fA-F]{6}$")
REF_RE = re.compile(r"^\{palette\.[A-Za-z][A-Za-z0-9]*\}$")
ID_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")


class ParseError(Exception):
    pass


class Validator:
    def __init__(self) -> None:
        self.errors: list[str] = []

    def error(self, path: Path, message: str) -> None:
        self.errors.append(f"{path}: {message}")


# 이 저장소의 제한된 스키마용 YAML parser로, 2칸 들여쓰기 mapping과 scalar를 처리한다.
# >-, >, |-, | 블록과 주석, 빈 줄도 지원한다.

def strip_inline_comment(line: str) -> str:
    in_single = False
    in_double = False
    out: list[str] = []
    i = 0
    while i < len(line):
        ch = line[i]
        if ch == "'" and not in_double:
            in_single = not in_single
        elif ch == '"' and not in_single:
            in_double = not in_double
        elif ch == "#" and not in_single and not in_double:
            break
        out.append(ch)
        i += 1
    return "".join(out).rstrip()


def parse_scalar(value: str) -> Any:
    value = value.strip()
    if not value:
        return ""
    if (value.startswith('"') and value.endswith('"')) or (
        value.startswith("'") and value.endswith("'")
    ):
        return value[1:-1]
    if value.isdigit():
        return int(value)
    return value


def parse_yaml_subset(text: str) -> dict[str, Any]:
    raw_lines = text.splitlines()
    lines: list[tuple[int, str]] = []
    for idx, raw in enumerate(raw_lines, start=1):
        if not raw.strip() or raw.lstrip().startswith("#"):
            continue
        if "\t" in raw:
            raise ParseError(f"line {idx}: tabs are not supported")
        indent = len(raw) - len(raw.lstrip(" "))
        cleaned = strip_inline_comment(raw.strip())
        if not cleaned:
            continue
        lines.append((indent, cleaned))

    root: dict[str, Any] = {}
    stack: list[tuple[int, dict[str, Any]]] = [(-1, root)]
    i = 0
    while i < len(lines):
        indent, content = lines[i]
        if ":" not in content:
            raise ParseError(f"line {i + 1}: expected mapping entry")

        while len(stack) > 1 and indent <= stack[-1][0]:
            stack.pop()
        if indent % 2 != 0:
            raise ParseError(f"line {i + 1}: indentation must be multiples of 2")

        parent = stack[-1][1]
        key, rest = content.split(":", 1)
        key = key.strip()
        rest = rest.strip()

        if rest in {">", ">-", "|", "|-"}:
            block_indent = None
            block_lines: list[str] = []
            j = i + 1
            while j < len(lines):
                next_indent, next_content = lines[j]
                if next_indent <= indent:
                    break
                if block_indent is None:
                    block_indent = next_indent
                block_lines.append(next_content)
                j += 1
            if rest.startswith(">"):
                value = " ".join(line.strip() for line in block_lines).strip()
            else:
                value = "\n".join(block_lines)
            parent[key] = value
            i = j
            continue

        if rest == "":
            child: dict[str, Any] = {}
            parent[key] = child
            stack.append((indent, child))
        else:
            parent[key] = parse_scalar(rest)
        i += 1

    return root


def load_yaml(path: Path) -> dict[str, Any]:
    try:
        return parse_yaml_subset(path.read_text())
    except ParseError as e:
        raise ParseError(f"{path}: {e}") from e


def key_order(mapping: dict[str, Any]) -> list[str]:
    return list(mapping.keys())


def ensure_mapping(v: Validator, path: Path, node: Any, label: str) -> dict[str, Any] | None:
    if not isinstance(node, dict):
        v.error(path, f"{label} must be a mapping")
        return None
    return node


def compare_keys(v: Validator, path: Path, label: str, actual: dict[str, Any], expected: dict[str, Any]) -> None:
    actual_keys = key_order(actual)
    expected_keys = key_order(expected)
    if actual_keys != expected_keys:
        missing = [k for k in expected_keys if k not in actual]
        extra = [k for k in actual_keys if k not in expected]
        if missing:
            v.error(path, f"{label} missing keys: {', '.join(missing)}")
        if extra:
            v.error(path, f"{label} extra keys: {', '.join(extra)}")
        if [k for k in actual_keys if k in expected] != [k for k in expected_keys if k in actual]:
            v.error(path, f"{label} key order does not match template")


def validate_color_value(v: Validator, path: Path, label: str, value: Any) -> None:
    if not isinstance(value, str):
        v.error(path, f"{label} must be a string")
        return
    if not (HEX_RE.match(value) or REF_RE.match(value)):
        v.error(path, f"{label} must be #RRGGBB or {{palette.name}}, got: {value}")


def validate_meta(v: Validator, path: Path, meta: dict[str, Any], template_meta: dict[str, Any]) -> None:
    compare_keys(v, path, "meta", meta, template_meta)
    theme_id = meta.get("id")
    if not isinstance(theme_id, str) or not ID_RE.match(theme_id):
        v.error(path, "meta.id must be kebab-case")
    variant = meta.get("variant")
    if variant not in {"light", "dark"}:
        v.error(path, "meta.variant must be 'light' or 'dark'")
    for key in ["name", "family", "author", "description"]:
        if key in meta and not isinstance(meta[key], str):
            v.error(path, f"meta.{key} must be a string")


def validate_palette(v: Validator, path: Path, palette: dict[str, Any], template_palette: dict[str, Any]) -> None:
    compare_keys(v, path, "palette", palette, template_palette)
    for key, value in palette.items():
        validate_color_value(v, path, f"palette.{key}", value)


def validate_roles(
    v: Validator,
    path: Path,
    roles: dict[str, Any],
    template_roles: dict[str, Any],
) -> None:
    compare_keys(v, path, "roles", roles, template_roles)
    for group_name, template_group in template_roles.items():
        group = roles.get(group_name)
        if not isinstance(group, dict):
            v.error(path, f"roles.{group_name} must be a mapping")
            continue
        compare_keys(v, path, f"roles.{group_name}", group, template_group)
        for key, value in group.items():
            validate_color_value(v, path, f"roles.{group_name}.{key}", value)


def validate_theme(v: Validator, path: Path, template: dict[str, Any]) -> None:
    try:
        data = load_yaml(path)
    except ParseError as e:
        v.error(path, str(e))
        return

    if key_order(data) != key_order(template):
        v.error(path, "top-level key order does not match template")

    for key in key_order(template):
        if key not in data:
            v.error(path, f"missing top-level key: {key}")
    for key in key_order(data):
        if key not in template:
            v.error(path, f"unexpected top-level key: {key}")

    if data.get("version") != 1:
        v.error(path, "version must be 1")

    meta = ensure_mapping(v, path, data.get("meta"), "meta")
    palette = ensure_mapping(v, path, data.get("palette"), "palette")
    roles = ensure_mapping(v, path, data.get("roles"), "roles")

    if meta is not None:
        validate_meta(v, path, meta, template["meta"])
    if palette is not None:
        validate_palette(v, path, palette, template["palette"])
    if roles is not None:
        validate_roles(v, path, roles, template["roles"])



def discover_default_targets(repo_root: Path) -> list[Path]:
    return sorted((repo_root / "core").glob("*.yaml"))



def main() -> int:
    parser = argparse.ArgumentParser(description="Validate canonical theme schema files.")
    parser.add_argument(
        "paths",
        nargs="*",
        help="Theme YAML files to validate. Defaults to themes/core/*.yaml",
    )
    parser.add_argument(
        "--template",
        default=None,
        help="Path to template YAML. Defaults to themes/TEMPLATE.yaml beside this script.",
    )
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parent
    template_path = Path(args.template).resolve() if args.template else repo_root / "TEMPLATE.yaml"

    try:
        template = load_yaml(template_path)
    except ParseError as e:
        print(f"error: failed to parse template: {e}", file=sys.stderr)
        return 2

    targets = [Path(p).resolve() for p in args.paths] if args.paths else [p.resolve() for p in discover_default_targets(repo_root)]
    if not targets:
        print("No theme files found to validate.", file=sys.stderr)
        return 2

    v = Validator()
    for target in targets:
        validate_theme(v, target, template)

    if v.errors:
        print("Theme validation failed:\n", file=sys.stderr)
        for err in v.errors:
            print(f"- {err}", file=sys.stderr)
        return 1

    print(f"Validated {len(targets)} theme file(s) successfully.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
