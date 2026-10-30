"""Run agenix at login or when activation selects a different mount script."""

import argparse
import fcntl
import os
from pathlib import Path
import subprocess
import sys
import tempfile


def replace_file(path: Path, content: bytes, mode: int = 0o600) -> None:
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(content)
            os.fchmod(output.fileno(), mode)
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def run(args: argparse.Namespace) -> int:
    state = args.state_dir
    state.mkdir(mode=0o700, parents=True, exist_ok=True)
    with (state / "lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        return run_locked(args, lock.fileno())


def run_locked(args: argparse.Namespace, lock_fd: int) -> int:
    state = args.state_dir
    current = state / "mount-script"
    successful = state / "last-successful-script"

    if args.mode == "activate":
        wrapper = args.wrapper_source.read_bytes()
        if (
            not args.wrapper_path.exists()
            or args.wrapper_path.read_bytes() != wrapper
            or not os.access(args.wrapper_path, os.X_OK)
        ):
            replace_file(args.wrapper_path, wrapper, 0o755)
        replace_file(current, os.fsencode(args.mount_script))
        if (
            successful.exists()
            and successful.read_bytes() == current.read_bytes()
            and args.secrets_dir.is_dir()
        ):
            return 0

    script = os.fsdecode(current.read_bytes())
    successful.unlink(missing_ok=True)
    # The child retains the lock if activation is interrupted while it is mounting.
    result = subprocess.run([script], check=False, pass_fds=(lock_fd,))
    if result.returncode:
        print(
            f"agenix: mount command failed ({result.returncode}): {script}",
            file=sys.stderr,
        )
        return result.returncode if result.returncode > 0 else 1
    replace_file(successful, os.fsencode(script))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="mode", required=True)
    activation = commands.add_parser("activate")
    login = commands.add_parser("login")
    for command in (activation, login):
        command.add_argument("--state-dir", type=Path, required=True)
    for name in ("mount-script", "wrapper-source", "wrapper-path", "secrets-dir"):
        activation.add_argument(f"--{name}", type=Path, required=True)
    args = parser.parse_args()
    try:
        return run(args)
    except OSError as error:
        print(
            f"agenix: {args.mode} failed in {args.state_dir}: {error}", file=sys.stderr
        )
        return 1


if __name__ == "__main__":
    sys.exit(main())
