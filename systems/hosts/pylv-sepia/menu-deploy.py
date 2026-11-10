#!/usr/bin/env python3
"""Restricted stdin-only release deployment command for Menu by pylv."""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import sys
import tarfile
import tempfile
from typing import BinaryIO

SHA = re.compile(r"[0-9a-f]{40}(?:[0-9a-f]{24})?")
ARCHIVE_SHA = re.compile(r"[0-9a-f]{64}")
DEPLOY_COMMAND = re.compile(
    r"deploy (?P<revision>[0-9a-f]{40}(?:[0-9a-f]{24})?) "
    r"(?P<archive_sha>[0-9a-f]{64})"
)
ROLLBACK_COMMAND = re.compile(r"rollback (?P<revision>[0-9a-f]{40}(?:[0-9a-f]{24})?)")
REQUIRED = (
    "index.html",
    "404.html",
    "rss.xml",
    "sitemap-index.xml",
    "admin/index.html",
    "admin/config.yml",
)
MAX_ARCHIVE_BYTES = 256 * 1024 * 1024
MAX_FILES = 10_000
MAX_FILE_BYTES = 64 * 1024 * 1024
MAX_TOTAL_BYTES = 384 * 1024 * 1024
KEEP_RELEASES = 5


class DeployError(Exception):
    """Raised for a rejected deploy request."""


def command(original: str) -> tuple[str, str, str | None]:
    deploy_match = DEPLOY_COMMAND.fullmatch(original)
    if deploy_match:
        return "deploy", deploy_match["revision"], deploy_match["archive_sha"]
    rollback_match = ROLLBACK_COMMAND.fullmatch(original)
    if rollback_match:
        return "rollback", rollback_match["revision"], None
    raise DeployError("invalid deployment command")


def safe_name(name: str) -> Path:
    path = PurePosixPath(name)
    if (
        not name
        or path.is_absolute()
        or ".." in path.parts
        or any(part in ("", ".") for part in path.parts)
    ):
        raise DeployError("unsafe archive path")
    return Path(*path.parts)


def copy_stdin(destination: Path, stream: BinaryIO) -> str:
    digest = hashlib.sha256()
    total = 0
    with destination.open("xb") as output:
        while True:
            chunk = stream.read(1024 * 1024)
            if not chunk:
                break
            total += len(chunk)
            if total > MAX_ARCHIVE_BYTES:
                raise DeployError("archive exceeds size limit")
            digest.update(chunk)
            output.write(chunk)
    return digest.hexdigest()


def extract(archive: Path, staging: Path) -> dict[str, str]:
    manifest: dict[str, str] = {}
    total = 0
    count = 0
    paths: dict[str, str] = {}
    try:
        source = tarfile.open(archive, mode="r:*")
    except tarfile.TarError as error:
        raise DeployError("invalid archive") from error
    with source:
        for member in source:
            count += 1
            if count > MAX_FILES:
                raise DeployError("too many archive members")
            if member.isdir() and member.name in (".", "./"):
                continue
            relative = safe_name(member.name)
            name = relative.as_posix()
            if name == ".manifest.json":
                raise DeployError("reserved archive path")
            if name in paths:
                raise DeployError("duplicate archive path")
            ancestors = [
                parent.as_posix()
                for parent in relative.parents
                if parent.as_posix() != "."
            ]
            if any(paths.get(ancestor) == "file" for ancestor in ancestors):
                raise DeployError("archive path type collision")
            if member.isdir():
                paths[name] = "directory"
                continue
            if not member.isfile() or member.size < 0 or member.size > MAX_FILE_BYTES:
                raise DeployError("unsupported archive member")
            if any(existing.startswith(f"{name}/") for existing in paths):
                raise DeployError("archive path type collision")
            paths[name] = "file"
            total += member.size
            if total > MAX_TOTAL_BYTES:
                raise DeployError("archive content exceeds size limit")
            target = staging / relative
            target.parent.mkdir(mode=0o755, parents=True, exist_ok=True)
            payload = source.extractfile(member)
            if payload is None:
                raise DeployError("unreadable archive member")
            digest = hashlib.sha256()
            written = 0
            with target.open("xb") as output:
                while chunk := payload.read(1024 * 1024):
                    written += len(chunk)
                    if written > member.size:
                        raise DeployError("archive member size mismatch")
                    digest.update(chunk)
                    output.write(chunk)
                output.flush()
                os.fsync(output.fileno())
            if written != member.size:
                raise DeployError("truncated archive member")
            os.chmod(target, 0o644)
            manifest[name] = digest.hexdigest()
    if not all(name in manifest for name in REQUIRED):
        raise DeployError("archive lacks required site files")
    return manifest


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def release_files(release: Path) -> set[str] | None:
    files: set[str] = set()
    for directory, directory_names, file_names in os.walk(release, followlinks=False):
        parent = Path(directory)
        for name in directory_names:
            path = parent / name
            if stat.S_ISLNK(path.lstat().st_mode):
                return None
        for name in file_names:
            path = parent / name
            if not stat.S_ISREG(path.lstat().st_mode):
                return None
            relative = path.relative_to(release).as_posix()
            if relative != ".manifest.json":
                files.add(relative)
    return files


def valid_release(release: Path) -> bool:
    manifest_path = release / ".manifest.json"
    try:
        if (
            release.is_symlink()
            or not release.is_dir()
            or not stat.S_ISREG(manifest_path.lstat().st_mode)
        ):
            return False
        manifest = json.loads(manifest_path.read_text())
        archive_sha = manifest.get("archive_sha256")
        files = manifest.get("files")
        actual_files = release_files(release)
        if (
            not isinstance(archive_sha, str)
            or not ARCHIVE_SHA.fullmatch(archive_sha)
            or not isinstance(files, dict)
            or actual_files is None
            or set(files) != actual_files
            or not set(REQUIRED).issubset(files)
        ):
            return False
        for name, digest in files.items():
            if (
                not isinstance(name, str)
                or not isinstance(digest, str)
                or not ARCHIVE_SHA.fullmatch(digest)
            ):
                return False
            target = release / safe_name(name)
            if file_sha256(target) != digest:
                return False
        return True
    except (DeployError, OSError, ValueError, TypeError):
        return False


def sync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def sync_tree(root: Path) -> None:
    directories = [Path(directory) for directory, _, _ in os.walk(root)]
    for directory in reversed(directories):
        sync_directory(directory)


def switch_current(root: Path, release: Path) -> None:
    temporary = root / ".current.new"
    temporary.unlink(missing_ok=True)
    temporary.symlink_to(release.relative_to(root))
    os.replace(temporary, root / "current")
    sync_directory(root)


def prune(releases: Path, current: Path) -> None:
    current_target = current.resolve()
    candidates = sorted(
        (
            path
            for path in releases.iterdir()
            if SHA.fullmatch(path.name)
            and not path.is_symlink()
            and path.is_dir()
            and valid_release(path)
        ),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    for release in candidates[KEEP_RELEASES:]:
        if release.resolve() != current_target:
            shutil.rmtree(release)


def deploy(
    root: Path, revision: str, expected_archive_sha: str, stream: BinaryIO
) -> None:
    releases = root / "releases"
    releases.mkdir(mode=0o755, parents=True, exist_ok=True)
    lock_path = root / ".deploy.lock"
    with lock_path.open("a+") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        release = releases / revision
        if release.exists():
            if not valid_release(release):
                raise DeployError("existing release is invalid")
            manifest = json.loads((release / ".manifest.json").read_text())
            if manifest.get("archive_sha256") != expected_archive_sha:
                raise DeployError("release SHA already exists with another archive")
            switch_current(root, release)
            return
        with tempfile.TemporaryDirectory(prefix=".staging-", dir=releases) as temporary:
            staging = Path(temporary)
            archive = staging / "archive.tar"
            actual_archive_sha = copy_stdin(archive, stream)
            if actual_archive_sha != expected_archive_sha:
                raise DeployError("archive checksum mismatch")
            content = staging / "content"
            content.mkdir(mode=0o755)
            files = extract(archive, content)
            manifest_path = content / ".manifest.json"
            with manifest_path.open("x") as manifest_file:
                manifest_file.write(
                    json.dumps(
                        {"archive_sha256": actual_archive_sha, "files": files},
                        sort_keys=True,
                    )
                    + "\n"
                )
                manifest_file.flush()
                os.fsync(manifest_file.fileno())
            os.chmod(manifest_path, 0o644)
            sync_tree(content)
            os.replace(content, release)
            sync_directory(releases)
        switch_current(root, release)
        prune(releases, root / "current")


def rollback(root: Path, revision: str) -> None:
    with (root / ".deploy.lock").open("a+") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        release = root / "releases" / revision
        if not release.is_dir() or not valid_release(release):
            raise DeployError("requested release is invalid")
        switch_current(root, release)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--command")
    args = parser.parse_args()
    try:
        action, revision, archive_sha = command(
            args.command
            if args.command is not None
            else os.environ.get("SSH_ORIGINAL_COMMAND", "")
        )
        root = args.root.resolve()
        if action == "deploy":
            deploy(root, revision, archive_sha or "", sys.stdin.buffer)
        else:
            rollback(root, revision)
    except DeployError as error:
        print(f"menu deployment rejected: {error}", file=sys.stderr)
        return 1
    except (OSError, tarfile.TarError, EOFError, ValueError, TypeError):
        print(
            "menu deployment rejected: deployment operation failed",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
