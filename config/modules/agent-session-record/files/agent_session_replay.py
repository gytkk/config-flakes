from __future__ import annotations

import json
import os
import shlex
import subprocess
import sys
import tempfile
from collections.abc import Sequence
from datetime import UTC, datetime
from pathlib import Path

from agent_session_config import load_config
from agent_session_provider import ProviderAdapter


def run_replay(provider: ProviderAdapter, worker_command: Sequence[str]) -> int:
    os.umask(0o077)
    home = Path(os.environ.get("HOME", Path.home()))
    try:
        config = load_config(home)
    except ValueError as error:
        sys.stderr.write(f"{error}\n")
        return 1

    state_dir = Path(
        config.get(
            "AGENT_SESSION_RECORD_STATE_DIR",
            Path(config.get("XDG_STATE_HOME", home / ".local/state"))
            / "agent-session-record",
        )
    )
    sessions_dir = provider.sessions_dir(home, config)
    session_state_dir = state_dir / "sessions"
    temp_dir = state_dir / "tmp"
    scope = config.get("AGENT_SESSION_RECORD_SCOPE", "personal")
    if not sessions_dir.is_dir():
        sys.stderr.write(
            f"{provider.name} sessions directory missing: {sessions_dir}\n"
        )
        return 1
    session_state_dir.mkdir(parents=True, exist_ok=True)
    temp_dir.mkdir(parents=True, exist_ok=True)
    session_state_dir.chmod(0o700)
    temp_dir.chmod(0o700)

    sessions = provider.discover_replay_sessions(home, config)
    if not sessions:
        print(f"No {provider.name} session transcripts found under {sessions_dir}")
        return 0

    uploaded = 0
    failed = 0
    for session in sessions:
        if not session.session_id:
            sys.stderr.write(f"skip (missing session id): {session.transcript}\n")
            failed += 1
            continue
        state_file = (
            session_state_dir / f"{scope}-{provider.name}-{session.session_id}.json"
        )
        state_file.unlink(missing_ok=True)
        descriptor, raw_payload = tempfile.mkstemp(
            dir=temp_dir, prefix=f"{provider.name}-upload-"
        )
        os.close(descriptor)
        payload_path = Path(raw_payload)
        try:
            payload_path.write_text(
                json.dumps(provider.replay_payload(session), ensure_ascii=False) + "\n",
                encoding="utf-8",
            )
            result = subprocess.run(
                [
                    *worker_command,
                    "--mode",
                    "payload",
                    "--provider",
                    provider.name,
                    "--payload-file",
                    str(payload_path),
                ],
                stdin=subprocess.DEVNULL,
                check=False,
            )
            if result.returncode == 0 and state_file.is_file():
                print(f"uploaded: {session.session_id}")
                uploaded += 1
            else:
                sys.stderr.write(
                    f"upload not confirmed (queued or failed): {session.session_id}\n"
                )
                failed += 1
        finally:
            payload_path.unlink(missing_ok=True)

    print(f"\nprocessed: {len(sessions)}")
    print(f"uploaded: {uploaded}")
    print(f"failed: {failed}")
    print_remote_count(provider, config)
    print(f"completed at: {datetime.now(UTC).isoformat(timespec='seconds')}")
    return 1 if failed else 0


def print_remote_count(provider: ProviderAdapter, config: dict[str, str]) -> None:
    ssh = Path(config.get("AGENT_SESSION_RECORD_SSH_BIN", "/usr/bin")) / "ssh"
    remote = (
        f"{config.get('AGENT_SESSION_RECORD_REMOTE_USER', 'gytkk')}@"
        f"{config.get('AGENT_SESSION_RECORD_REMOTE_HOST', 'pylv-onyx')}"
    )
    remote_dir = (
        Path(
            config.get(
                "AGENT_SESSION_RECORD_REMOTE_BASE_PATH", "/home/gytkk/agent-sessions"
            )
        )
        / config.get("AGENT_SESSION_RECORD_SCOPE", "personal")
        / provider.name
    )
    result = subprocess.run(
        [
            str(ssh),
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            remote,
            (
                f"find {shlex.quote(str(remote_dir))} -type f -name '*.jsonl' "
                "2>/dev/null | wc -l"
            ),
        ],
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode == 0 and result.stdout.strip():
        print(f"remote jsonl count: {result.stdout.strip()}")
    else:
        print("remote jsonl count: unavailable")
