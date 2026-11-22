#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# ///

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import tempfile
from collections.abc import Sequence
from datetime import UTC, datetime
from pathlib import Path

from agent_session_config import load_config, provider_enabled
from agent_session_provider import ProviderAdapter


def warn(path: Path, command: str, message: str) -> None:
    line = f"{datetime.now(UTC).isoformat(timespec='seconds')} {command}: {message}\n"
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("a", encoding="utf-8") as handle:
            handle.write(line)
        path.chmod(0o600)
    except OSError:
        pass


def claude_no_persistence(pid: int, proc: Path = Path("/proc")) -> bool:
    """Recognize the managed native Linux CLI, never an arbitrary ancestor's flag."""
    if sys.platform != "linux":
        return False
    try:
        for _ in range(4):
            if pid <= 1:
                break
            process = proc / str(pid)
            exe = os.readlink(process / "exe")
            with (process / "cmdline").open("rb") as handle:
                data = handle.read(65537)
            if len(data) > 65536 or not data.endswith(b"\0"):
                return False
            args = data[:-1].split(b"\0")[1:]
            if re.fullmatch(
                r"/nix/store/[a-z0-9]{32}-claude-code-[^/]+/bin/(?:claude|\.claude-wrapped)",
                exe,
            ):
                for arg in args:
                    if arg == b"--no-session-persistence":
                        return True
                    # Stop before a prompt, option value, delimiter, or unknown grammar.
                    if arg not in (b"-p", b"--print", b"--verbose"):
                        break
                return False
            if Path(exe).name not in {"sh", "bash", "dash", "zsh"} or args != [
                b"-c",
                b"~/.local/bin/agent-session-record hook claude session-end",
            ]:
                return False
            parent = re.search(
                r"^PPid:\s*(\d+)$", (process / "status").read_text(), re.MULTILINE
            )
            if not parent:
                break
            pid = int(parent[1])
    except (OSError, ValueError):
        pass
    return False


def run_hook(
    provider: ProviderAdapter, event: str, worker_command: Sequence[str]
) -> int:
    os.umask(0o077)
    contract = provider.hook_contract(event)
    command = f"{provider.name}-{event}"
    home = Path(os.environ.get("HOME", Path.home()))
    default_state_dir = (
        Path(os.environ.get("XDG_STATE_HOME", home / ".local/state"))
        / "agent-session-record"
    )
    try:
        config = load_config(home)
    except ValueError as error:
        warn(default_state_dir / "warnings.log", command, str(error))
        if contract.returns_continue:
            print('{"continue":true}')
        return 0
    state_dir = Path(
        config.get(
            "AGENT_SESSION_RECORD_STATE_DIR",
            Path(config.get("XDG_STATE_HOME", home / ".local/state"))
            / "agent-session-record",
        )
    )
    warning_log = state_dir / "warnings.log"
    debug_log = state_dir / "debug.log"
    if not provider_enabled(config, provider.name):
        if contract.returns_continue:
            print('{"continue":true}')
        return 0

    try:
        descriptor, raw_payload = tempfile.mkstemp(prefix=f"{command}.")
        payload = Path(raw_payload)
        with os.fdopen(descriptor, "wb") as handle:
            data = sys.stdin.buffer.read()
            if provider.name == "claude":
                try:
                    body = json.loads(data)
                except (ValueError, UnicodeError):
                    body = None  # Let the worker record the invalid payload.
                if isinstance(body, dict):
                    body.pop("_verified_no_persistence", None)
                    if claude_no_persistence(os.getppid()):
                        body["_verified_no_persistence"] = True
                    data = json.dumps(body).encode()
            handle.write(data)
    except OSError as error:
        warn(warning_log, command, f"failed to persist hook payload: {error}")
        if contract.returns_continue:
            print('{"continue":true}')
        return 0

    arguments = [
        *worker_command,
        "--mode",
        contract.mode,
        "--provider",
        provider.name,
        "--payload-file",
        str(payload),
    ]
    try:
        debug_log.parent.mkdir(parents=True, exist_ok=True)
        debug_handle = debug_log.open("ab")
        debug_log.chmod(0o600)
        try:
            subprocess.Popen(
                arguments,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=debug_handle,
                start_new_session=True,
                close_fds=True,
            )
        finally:
            debug_handle.close()
    except OSError as error:
        payload.unlink(missing_ok=True)
        warn(warning_log, command, f"failed to start worker: {error}")
    if contract.returns_continue:
        print('{"continue":true}')
    return 0
