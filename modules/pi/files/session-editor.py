"""Open a checkout document in a session-owned Neovim terminal on this core."""

import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import time
from urllib.parse import unquote, urlsplit


def document(raw, cwd):
    url = urlsplit(raw)
    if url.scheme not in ("", "file") or url.netloc not in ("", "localhost") or url.query:
        return None
    name = unquote(url.path, errors="strict")
    if not name or any(ord(char) < 32 or ord(char) == 127 for char in raw + name):
        return None
    path = (cwd / name).resolve()
    if not path.is_relative_to(cwd) or path.suffix.lower() != ".md" or not path.is_file():
        return None
    anchor = unquote(url.fragment)
    line = 1
    if re.fullmatch(r"L[1-9][0-9]{0,7}", anchor):
        line = int(anchor[1:])
    elif anchor:
        with path.open(encoding="utf-8") as stream:
            for number, text in enumerate(stream, 1):
                match = re.match(r"^ {0,3}#{1,6}\s+(.+?)\s*#*\s*$", text)
                if match and re.sub(r"[^\w -]", "", match[1].lower()).replace(" ", "-") == anchor:
                    line = number
                    break
    return path, line


def output(*args):
    return subprocess.check_output(args, text=True, timeout=8).strip()


def open_document(parent, session, path, line):
    key = hashlib.sha256(f"{parent}:{session}".encode()).hexdigest()[:20]
    name = f"pi-vim-{key}"
    private = Path(f"/tmp/pi-editor-{os.getuid()}")
    private.mkdir(mode=0o700, exist_ok=True)
    info = private.lstat()
    if info.st_uid != os.getuid() or info.st_mode != (stat.S_IFDIR | 0o700):
        raise ValueError("Unsafe Pi editor socket directory")
    socket = str(private / key)
    with (private / f"{key}.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if not Path(socket).exists():
            output("hc", "run", "--bg", "--no-notify", "--rm", "--parent", parent, "--name", name,
                   "--cwd", str(Path.cwd()), "--", "nvim", "--listen", socket, f"+{line}", "--", str(path))
            for _ in range(100):
                if Path(socket).is_socket():
                    break
                time.sleep(0.05)
            else:
                raise RuntimeError("Neovim did not create its session socket")
        else:
            # fnameescape handles Ex metacharacters; doubled quotes keep the path a Vim string literal.
            literal = "'" + str(path).replace("'", "''") + "'"
            output("nvim", "--server", socket, "--remote-expr",
                   f"execute('hide edit ' .. fnameescape({literal})) .. cursor({line}, 1)")
        return json.loads(output("hc", "get", name))["address"]


if __name__ == "__main__":
    parent, session, raw = sys.argv[1:]
    if not re.fullmatch(r"t[0-9a-z]+", parent) or not re.fullmatch(r"[a-f0-9-]{36}", session):
        sys.exit(2)
    target = document(raw, Path.cwd())
    if target is None:
        sys.exit(2)
    print(open_document(parent, session, *target))
