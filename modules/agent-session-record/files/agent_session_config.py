from __future__ import annotations

import json
import os
import re
import shlex
from pathlib import Path


def read_shell_config(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    if not path.is_file():
        return values
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, raw_value = line.split("=", 1)
        if not re.fullmatch(r"[A-Z][A-Z0-9_]*", key):
            continue
        try:
            parsed = shlex.split(raw_value, posix=True)
        except ValueError:
            continue
        if len(parsed) == 1:
            values[key] = parsed[0]
        elif not parsed and raw_value == "":
            values[key] = ""
    return values


def load_config(
    home: Path, environ: dict[str, str] | None = None
) -> dict[str, str]:
    values = dict(os.environ if environ is None else environ)
    config_dir = home / ".config/agent-session-record"
    json_path = config_dir / "config.json"
    if json_path.is_file():
        try:
            config = json.loads(json_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise ValueError(f"failed to load configuration {json_path}: {error}") from error
        if not isinstance(config, dict) or not all(
            isinstance(key, str) and isinstance(value, str)
            for key, value in config.items()
        ):
            raise ValueError(
                f"configuration must contain only string values: {json_path}"
            )
        values.update(config)
        return values
    values.update(read_shell_config(config_dir / "config.sh"))
    return values
