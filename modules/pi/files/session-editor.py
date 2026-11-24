"""Resolve a clicked file on the Pi host before Herdr splits the source pane."""

import json
from pathlib import Path
import re
import sys
from urllib.parse import unquote, urlsplit


def document(raw, cwd):
    if not raw or any(ord(char) < 32 or ord(char) == 127 for char in raw):
        return None

    def existing(name):
        if not name or any(ord(char) < 32 or ord(char) == 127 for char in name):
            return None
        path = (cwd / name).resolve()
        return path if path.is_relative_to(cwd) and path.is_file() else None

    uri = raw.lower().startswith("file:")
    if not uri:
        # A real filename can contain URL punctuation or end in a diagnostic suffix.
        literal = existing(raw)
        if literal:
            return literal, 1
        diagnostic = re.fullmatch(r"(.+?):([1-9][0-9]{0,7})(?::[1-9][0-9]{0,7})?", raw)
        if diagnostic and (path := existing(diagnostic[1])):
            return path, int(diagnostic[2])
    url = urlsplit(raw)
    if url.scheme not in ("", "file") or url.netloc not in ("", "localhost") or url.query:
        return None
    name = unquote(url.path, errors="strict")
    path = existing(name)
    if not path:
        return None
    anchor = unquote(url.fragment)
    if re.fullmatch(r"L[1-9][0-9]{0,7}", anchor):
        return path, int(anchor[1:])
    if anchor and path.suffix.lower() == ".md":
        with path.open(encoding="utf-8", errors="replace") as stream:
            for number, text in enumerate(stream, 1):
                match = re.match(r"^ {0,3}#{1,6}\s+(.+?)\s*#*\s*$", text)
                if match and re.sub(r"[^\w -]", "", match[1].lower()).replace(" ", "-") == anchor:
                    return path, number
    return path, 1


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(2)
    target = document(sys.argv[1], Path.cwd())
    if target is None:
        sys.exit(2)
    path, line = target
    print(json.dumps({"path": str(path), "line": line}))
