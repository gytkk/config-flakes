"""Route OMP file links to a reusable Neovim pane in the local Herdr App."""

import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import socket
import stat
import subprocess
import sys
import time
from urllib.parse import parse_qs, unquote, urlsplit

SCHEME = "omp-nvim"
OPEN_LUA = """(function()
  local path, line, col = unpack(_A)
  vim.cmd.stopinsert()
  local buffer = vim.fn.bufadd(path)
  vim.bo[buffer].buflisted = true
  vim.api.nvim_cmd({cmd = 'buffer', args = {tostring(buffer)}, mods = {hide = true}}, {})
  if line > 0 then
    line = math.min(line, vim.api.nvim_buf_line_count(0))
    local text = vim.api.nvim_buf_get_lines(0, line - 1, line, false)[1]
    vim.api.nvim_win_set_cursor(0, {line, math.min(math.max(col - 1, 0), #text)})
    vim.cmd('normal! zz')
  end
  return vim.api.nvim_buf_get_name(0)
end)()"""


def run(argv, **kwargs):
    return subprocess.run(argv, check=True, capture_output=True, text=True, timeout=15, **kwargs).stdout.strip()


def app_binary():
    path = shutil.which("herdr-app", path=os.pathsep.join([
        str(Path.home() / ".local/bin"), os.environ.get("PATH", ""),
    ]))
    if not path:
        raise ValueError("herdr-app is not installed")
    return path


def local_panes(app):
    return [pane for space in json.loads(run([app, "ls"]))["spaces"] if not space.get("machine")
            for tab in space["tabs"] for pane in tab["terminals"]]


def host_id():
    return hashlib.sha256(socket.gethostname().encode()).hexdigest()[:16]


def launch(executable, args):
    # Do not change file links in SSH, Herdr TUI, or ordinary terminal sessions.
    terminal = os.environ.get("HC_TERMINAL_ID")
    if terminal and not os.environ.get("SSH_CONNECTION") and not os.environ.get("HERDR_PANE_ID"):
        try:
            pane = next(p for p in local_panes(app_binary()) if p["terminal"] == terminal)
            if re.fullmatch(r"p[0-9a-f]+", pane["pane"]):
                os.environ["PI_LINK_SCHEME"] = SCHEME
                os.environ["PI_LINK_HOST"] = f"{host_id()}.{pane['pane']}"
        except (OSError, ValueError, StopIteration, subprocess.SubprocessError) as error:
            print(f"omp file links: using ordinary links ({error})", file=sys.stderr)
    os.execv(executable, [executable, *args])


def parse_link(url):
    parts = urlsplit(url)
    if parts.scheme != SCHEME or not re.fullmatch(r"[0-9a-f]{16}\.p[0-9a-f]+", parts.netloc):
        raise ValueError("Not an OMP Neovim file link")
    host, pane = parts.netloc.split(".")
    if host != host_id():
        raise ValueError("This file link belongs to another computer")
    if re.search(r"%(?![0-9a-fA-F]{2})", parts.path):
        raise ValueError("Invalid percent escape in file path")
    path = unquote(parts.path, errors="strict")
    if not path.startswith("/") or any(ord(c) < 32 or ord(c) == 127 for c in path):
        raise ValueError("Expected an absolute file path without control characters")
    query = parse_qs(parts.query, keep_blank_values=True)
    fragment = re.fullmatch(r"L([1-9][0-9]*)(?:C([1-9][0-9]*))?(?:-L[1-9][0-9]*)?", parts.fragment)
    positions = []
    for index, name in enumerate(("line", "col"), 1):
        values = query.get(name, [fragment[index] if fragment and fragment[index] else "0"])
        if len(values) != 1 or not re.fullmatch(r"[0-9]+", values[0]) or int(values[0]) > 2**31 - 1:
            raise ValueError(f"Invalid {name} position")
        positions.append(int(values[0]))
    return pane, Path(path).resolve(strict=True), *positions


def is_text(path):
    if not path.is_file():
        return False
    mime = run(["/usr/bin/file", "--brief", "--mime-type", "--", str(path)])
    return mime.startswith("text/") or mime.endswith(("+json", "+xml")) or mime in {
        "application/json", "application/xml", "application/javascript", "application/x-javascript",
        "application/x-empty", "inode/x-empty", "application/toml", "application/yaml", "application/x-yaml",
        "application/sql", "application/x-shellscript",
    }


def runtime_directory():
    # Short enough for macOS's Unix socket limit; never follow a pre-created symlink.
    path = Path(f"/tmp/omp-nvim-{os.getuid()}")
    path.mkdir(mode=0o700, exist_ok=True)
    info = path.lstat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != 0o700:
        raise ValueError(f"Unsafe Neovim runtime directory: {path}")
    return path


def remote(nvim, address, path, line, col):
    data = json.dumps([str(path), line, col], ensure_ascii=False).replace("'", "''")
    lua = OPEN_LUA.replace("'", "''")
    expression = f"json_encode(luaeval('{lua}', json_decode('{data}')))"
    result = run([nvim, "--server", str(address), "--remote-expr", expression])
    if Path(json.loads(result)).resolve() != path.resolve():
        raise ValueError(f"Neovim did not open the requested file: {path}")


def open_link(url, nvim):
    source, path, line, col = parse_link(url)
    if not path.is_file() and not path.is_dir():
        raise ValueError("Only regular files and directories can be opened")
    if not is_text(path):
        run(["/usr/bin/open", str(path)])
        return
    app = app_binary()
    root = runtime_directory()
    with (root / f"{source}.lock").open("w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        panes = local_panes(app)
        if not any(p["pane"] == source for p in panes):
            raise ValueError("The originating Herdr pane is no longer open")
        state = root / f"{source}.json"
        address = root / f"{source}.sock"
        saved = json.loads(state.read_text()) if state.exists() else {}
        target = next((p for p in panes if p["pane"] == saved.get("pane")
                       and p["terminal"] == saved.get("terminal")), None)
        if target is None:
            address.unlink(missing_ok=True)
            target = json.loads(run([app, "do", "pane.split", json.dumps({
                "direction": "right", "pane": source,
                "argv": [nvim, "--listen", str(address), "--", str(path)],
            })]))
            state.write_text(json.dumps(target))
        # A new pane's process may still be starting. Never send shell keystrokes.
        deadline = time.monotonic() + 10
        while not address.exists() and time.monotonic() < deadline:
            time.sleep(0.05)
        if not address.exists():
            raise ValueError("Neovim did not start its server; inspect the editor pane")
        remote(nvim, address, path, line, col)
        run([app, "do", "pane.focus", json.dumps({"pane": target["pane"]})])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    launcher = commands.add_parser("launch")
    launcher.add_argument("executable")
    launcher.add_argument("args", nargs=argparse.REMAINDER)
    opener = commands.add_parser("open")
    opener.add_argument("--nvim", required=True)
    opener.add_argument("url")
    args = parser.parse_args()
    try:
        if args.command == "launch":
            launch(args.executable, args.args)
        else:
            open_link(args.url, args.nvim)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        message = error.stderr if isinstance(error, subprocess.CalledProcessError) else str(error)
        print(f"OMP file opener: {message}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
