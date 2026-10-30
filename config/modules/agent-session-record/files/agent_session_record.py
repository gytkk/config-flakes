#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# ///

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from agent_session_hook import run_hook
from agent_session_provider import PROVIDERS, get_provider
from agent_session_replay import run_replay
from agent_session_upload_worker import run_worker


def parse_arguments(arguments: list[str] | None = None) -> argparse.Namespace:
    raw_arguments = sys.argv[1:] if arguments is None else arguments
    if raw_arguments[:1] == ["_worker"]:
        return parse_worker_arguments(raw_arguments[1:])

    parser = argparse.ArgumentParser(prog="agent-session-record")
    commands = parser.add_subparsers(dest="command", required=True)

    hook = commands.add_parser("hook", help="process an agent hook event")
    hook_providers = hook.add_subparsers(dest="provider", required=True)
    for name, provider in PROVIDERS.items():
        hook_provider = hook_providers.add_parser(name)
        hook_provider.add_argument("event", choices=provider.hook_events)

    replay = commands.add_parser("replay", help="capture all provider history once")
    replay.add_argument("provider", choices=PROVIDERS)

    return parser.parse_args(raw_arguments)


def parse_worker_arguments(arguments: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(prog="agent-session-record _worker")
    parser.add_argument(
        "--mode", choices=("payload", "session-start-sweep"), default="payload"
    )
    parser.add_argument("--provider", choices=PROVIDERS, required=True)
    parser.add_argument("--payload-file", type=Path)
    parsed = parser.parse_args(arguments)
    parsed.command = "_worker"
    if parsed.mode == "payload" and parsed.payload_file is None:
        parser.error("--payload-file is required in payload mode")
    return parsed


def worker_command() -> list[str]:
    return [sys.executable, str(Path(__file__).resolve()), "_worker"]


def main(arguments: list[str] | None = None) -> int:
    parsed = parse_arguments(arguments)
    try:
        provider = get_provider(parsed.provider)
        if parsed.command == "hook":
            return run_hook(provider, parsed.event, worker_command())
        if parsed.command == "replay":
            return run_replay(provider, worker_command())
        return run_worker(provider.name, parsed.mode, parsed.payload_file)
    except ValueError as error:
        sys.stderr.write(f"agent-session-record: {error}\n")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
