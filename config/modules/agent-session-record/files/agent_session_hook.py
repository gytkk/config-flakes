#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# ///

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from collections.abc import Sequence
from datetime import UTC, datetime
from pathlib import Path

from agent_session_config import load_config
from agent_session_provider import PROVIDERS, ProviderAdapter


def warn(path: Path, command: str, message: str) -> None:
    line = f"{datetime.now(UTC).isoformat(timespec='seconds')} {command}: {message}\n"
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("a", encoding="utf-8") as handle:
            handle.write(line)
        path.chmod(0o600)
    except OSError:
        pass


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
    enabled_providers = {
        name.strip()
        for name in config.get(
            "AGENT_SESSION_RECORD_ENABLED_PROVIDERS", ",".join(PROVIDERS)
        ).split(",")
        if name.strip()
    }
    if provider.name not in enabled_providers:
        if contract.returns_continue:
            print('{"continue":true}')
        return 0

    try:
        descriptor, raw_payload = tempfile.mkstemp(prefix=f"{command}.")
        payload = Path(raw_payload)
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(sys.stdin.buffer.read())
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
