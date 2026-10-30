"""Tests for the constrained Menu by pylv release deploy command."""

from __future__ import annotations

import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("menu-deploy.py")
SHA = "a" * 40
OTHER_SHA = "b" * 40
REQUIRED = {
    "index.html": b"home",
    "404.html": b"missing",
    "rss.xml": b"rss",
    "sitemap-index.xml": b"sitemap",
    "admin/index.html": b"admin",
    "admin/config.yml": b"backend: github\n",
}


def archive(files: dict[str, bytes], extra: tuple[str, str] | None = None) -> bytes:
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w") as tar:
        for name, data in files.items():
            entry = tarfile.TarInfo(name)
            entry.size = len(data)
            tar.addfile(entry, io.BytesIO(data))
        if extra:
            entry = tarfile.TarInfo(extra[0])
            entry.type = getattr(tarfile, extra[1])
            tar.addfile(entry)
    return output.getvalue()


class DeployTests(unittest.TestCase):
    def run_command(
        self, root: Path, original: str, payload: bytes = b""
    ) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--root", str(root), "--command", original],
            input=payload,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def deploy(
        self, root: Path, revision: str = SHA, files: dict[str, bytes] | None = None
    ) -> bytes:
        payload = archive(files or REQUIRED)
        result = self.run_command(
            root, f"deploy {revision} {hashlib.sha256(payload).hexdigest()}", payload
        )
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        return payload

    def test_deploy_manifest_and_rollback(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.deploy(root, SHA)
            self.deploy(
                root, OTHER_SHA, {**REQUIRED, "recipe/index.html": b"recipe"}
            )
            result = self.run_command(root, f"rollback {SHA}")
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            self.assertEqual((root / "current").resolve().name, SHA)
            manifest = json.loads(
                (root / "releases" / SHA / ".manifest.json").read_text()
            )
            self.assertIn("index.html", manifest["files"])

    def test_existing_sha_requires_identical_archive(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.deploy(root)
            payload = archive({**REQUIRED, "different.html": b"x"})
            result = self.run_command(
                root, f"deploy {SHA} {hashlib.sha256(payload).hexdigest()}", payload
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"another archive", result.stderr)

    def test_rejects_checksum_command_and_missing_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            payload = archive(REQUIRED)
            result = self.run_command(root, f"deploy {SHA} {'0' * 64}", payload)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"checksum", result.stderr)
            result = self.run_command(root, "deploy x /tmp/x", payload)
            self.assertNotEqual(result.returncode, 0)
            result = self.run_command(
                root,
                f"deploy  {SHA} {hashlib.sha256(payload).hexdigest()}",
                payload,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"invalid deployment command", result.stderr)
            missing = archive({"index.html": b"x"})
            result = self.run_command(
                root, f"deploy {SHA} {hashlib.sha256(missing).hexdigest()}", missing
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"required", result.stderr)

    def test_rejects_traversal_duplicate_links_and_reserved_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for files, extra in [
                ({**REQUIRED, "../escape": b"x"}, None),
                ({**REQUIRED, ".manifest.json": b"forged"}, None),
                (REQUIRED, ("link", "SYMTYPE")),
            ]:
                payload = archive(files, extra)
                result = self.run_command(
                    root, f"deploy {SHA} {hashlib.sha256(payload).hexdigest()}", payload
                )
                self.assertNotEqual(result.returncode, 0)
            output = io.BytesIO()
            with tarfile.open(fileobj=output, mode="w") as tar:
                for name, data in REQUIRED.items():
                    entry = tarfile.TarInfo(name)
                    entry.size = len(data)
                    tar.addfile(entry, io.BytesIO(data))
                duplicate = tarfile.TarInfo("index.html")
                duplicate.size = 1
                tar.addfile(duplicate, io.BytesIO(b"x"))
            payload = output.getvalue()
            result = self.run_command(
                root, f"deploy {SHA} {hashlib.sha256(payload).hexdigest()}", payload
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"duplicate", result.stderr)

    def test_rejects_file_directory_prefix_collisions_without_traceback(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for files in [
                {"admin": b"file", **REQUIRED},
                {**REQUIRED, "admin": b"file"},
            ]:
                payload = archive(files)
                result = self.run_command(
                    root,
                    f"deploy {SHA} {hashlib.sha256(payload).hexdigest()}",
                    payload,
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn(b"Traceback", result.stderr)

    def test_rejects_invalid_or_tampered_rollback(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = self.run_command(root, f"rollback {SHA}")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"invalid", result.stderr)
            self.deploy(root)
            release = root / "releases" / SHA
            (release / "index.html").write_bytes(b"tampered")
            result = self.run_command(root, f"rollback {SHA}")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"invalid", result.stderr)

            self.deploy(root, OTHER_SHA)
            other_release = root / "releases" / OTHER_SHA
            (other_release / ".manifest.json").write_text(
                json.dumps({"archive_sha256": "0" * 64, "files": {}})
            )
            result = self.run_command(root, f"rollback {OTHER_SHA}")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"invalid", result.stderr)

    def test_rejects_symlink_added_after_deploy(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.deploy(root)
            release = root / "releases" / SHA
            (release / "extra-link").symlink_to("index.html")
            result = self.run_command(root, f"rollback {SHA}")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"invalid", result.stderr)

    def test_pruning_only_removes_valid_sha_releases(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            unrelated = root / "releases" / "do-not-delete"
            unrelated.mkdir(parents=True)
            for index in range(6):
                revision = f"{index + 1:040x}"
                self.deploy(
                    root, revision, {**REQUIRED, "release.txt": str(index).encode()}
                )
            releases = root / "releases"
            retained = [
                path for path in releases.iterdir() if path.name != "do-not-delete"
            ]
            self.assertLessEqual(len(retained), 5)
            self.assertTrue(unrelated.is_dir())


if __name__ == "__main__":
    unittest.main()
