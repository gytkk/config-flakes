"""Merge managed shortcuts, pane spacing, and the prefix mod without owning app state."""

import argparse
import json
import os
from pathlib import Path
import shutil
import tempfile


def replace(path: Path, content: bytes) -> bool:
    if path.exists() and path.read_bytes() == content:
        return False
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        shutil.copy2(path, path.with_name(path.name + ".home-manager.bak"))
    fd, name = tempfile.mkstemp(prefix=".home-manager-", dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as file:
            file.write(content)
        os.replace(name, path)
    finally:
        if os.path.exists(name):
            os.unlink(name)
    return True


def status_layout(home: Path) -> dict:
    target = home / "layouts/tui.json"
    layout = json.loads((target if target.exists() else home / "layouts/main.json").read_text())
    pending = [layout]
    has_status = False
    while pending:
        node = pending.pop()
        content = node.get("content", {})
        if node.get("id") == "status" and content.get("mod") in ("workspace-status", "tui-prefix"):
            content["mod"] = "tui-prefix"
            has_status = True
        if content.get("tab") == "active":
            content.setdefault("pane", {})["gap"] = 4
        pending.extend(node.get("children", []))
    if not has_status:
        raise ValueError("Herdr App layout has no compatible status block; leaving configuration unchanged")
    return layout


def install(source: Path, home: Path) -> None:
    keys_path = home / "keys.json"
    if not keys_path.exists():
        print(f"Herdr App keys not found; launch the app once, then apply again: {keys_path}")
        return
    current = json.loads(keys_path.read_text())
    managed = json.loads((source / "keys.json").read_text())
    if not isinstance(current, dict) or any(not isinstance(section, dict) for section in current.values()):
        raise ValueError(f"Expected keybinding section objects in {keys_path}")
    layout = status_layout(home)
    for section, bindings in managed.items():
        current.setdefault(section, {}).update(bindings)
    # Install the action's mod before publishing the shortcut that opens it.
    for path in sorted((source / "mods").rglob("*")):
        if path.is_file() and path.name in {"main.js", "mod.json"}:
            replace(home / path.relative_to(source), path.read_bytes())
    replace(home / "layouts/tui.json", (json.dumps(layout, indent=2) + "\n").encode())
    if replace(keys_path, (json.dumps(current, indent=2, ensure_ascii=False) + "\n").encode()):
        print(f"Updated Herdr App shortcuts: {keys_path}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("home", type=Path)
    args = parser.parse_args()
    install(args.source, args.home)
