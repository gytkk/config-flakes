#!/usr/bin/env python3
"""Check local Markdown links and repository-root paths in current documents.

Only simple inline Markdown links and concrete repository paths in inline or
fenced code are checked. Reference-style links, anchors, URLs, shell
variables, globs, absolute paths, and home-directory paths are outside this
checker's scope.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DOCUMENTS = (
    ROOT / "README.md",
    ROOT / "AGENTS.md",
)
REPOSITORY_DIRS = (
    "config|agent-core|base|docs|hosts|lib|modules|overlays|packages|scripts|secrets|systems|themes"
)
INLINE_LINK = re.compile(r"(?<!!)\[[^]]*\]\(([^)]+)\)")
INLINE_CODE = re.compile(r"`([^`]+)`")
FENCE = re.compile(r"^\s*(`{3,}|~{3,})")
REPOSITORY_PATH = re.compile(rf"(?<![\w./:$~*-])((?:{REPOSITORY_DIRS})/\S+)")
FLAKE_UPDATE_INPUT = re.compile(r"\bnix flake update\s+(?:[\w][\w./-]*\s+)*$")


def is_skipped(target: str) -> bool:
    return (
        not target
        or target.startswith(("#", "<", "$", "~", "/"))
        or "://" in target
        or target.startswith(("mailto:", "data:"))
        or any(
            character in target
            for character in ("<", ">", "$", "*", "?", "[", "]", "{", "}")
        )
    )


def link_target(raw_target: str) -> str:
    target = raw_target.strip()
    if not target:
        return ""
    if target.startswith("<") and ">" in target:
        target = target[1 : target.index(">")]
    else:
        target = target.split(maxsplit=1)[0]
    return target.split("#", maxsplit=1)[0]


def report_missing(
    document: Path, line_number: int, target: str, errors: list[str]
) -> None:
    errors.append(f"{display_path(document)}:{line_number}: missing path: {target}")


def display_path(path: Path) -> Path | str:
    try:
        return path.relative_to(ROOT)
    except ValueError:
        return path


def check_document(document: Path) -> list[str]:
    errors: list[str] = []
    if not document.is_file():
        errors.append(f"{display_path(document)}: missing document")
        return errors

    try:
        lines = document.read_text().splitlines()
    except OSError as error:
        errors.append(f"{display_path(document)}: unable to read document: {error}")
        return errors

    fence_marker = ""
    for line_number, line in enumerate(lines, start=1):
        fence = FENCE.match(line)
        if fence:
            marker = fence.group(1)
            if not fence_marker:
                fence_marker = marker
                continue
            if (
                marker[0] == fence_marker[0]
                and len(marker) >= len(fence_marker)
                and not line[fence.end() :].strip()
            ):
                fence_marker = ""
                continue

        if not fence_marker:
            for match in INLINE_LINK.finditer(line):
                target = link_target(match.group(1))
                if not is_skipped(target) and not (document.parent / target).exists():
                    report_missing(document, line_number, target, errors)

        code_chunks = [line] if fence_marker else INLINE_CODE.findall(line)
        for code in code_chunks:
            for match in REPOSITORY_PATH.finditer(code):
                # Positional flake update arguments name inputs, not filesystem paths.
                if FLAKE_UPDATE_INPUT.search(code[: match.start()]):
                    continue
                raw_target = match.group(1)
                if is_skipped(raw_target):
                    continue
                target = raw_target.rstrip(".,;:!?)]}`'\"")
                if not (ROOT / target).exists():
                    report_missing(document, line_number, target, errors)

    return errors


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Check simple local Markdown links and repository-root paths.",
        epilog=(
            "Limits: does not validate anchors, reference-style links, remote URLs, "
            "variables, globs, absolute paths, or home-directory paths."
        ),
    )
    parser.add_argument(
        "documents", nargs="*", type=Path, help="Markdown documents to check"
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    documents = (
        [path.resolve() for path in args.documents]
        if args.documents
        else list(DEFAULT_DOCUMENTS)
    )
    errors = [error for document in documents for error in check_document(document)]
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"Checked {len(documents)} document(s).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
