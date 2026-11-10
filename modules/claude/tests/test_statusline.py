"""Keep status line Git labels isolated between working directories."""

import json
import os
import re
import subprocess
import tempfile
import time
import unittest
from pathlib import Path

MODULE = Path(__file__).resolve().parents[1]


class StatuslineTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="claude-statusline-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.cache = self.root / "cache"
        self.cache.mkdir()
        self.env = {
            **os.environ,
            "TMPDIR": str(self.cache),
            "HERDR_ENV": "0",
            "COLUMNS": "0",
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_AUTHOR_NAME": "Statusline Test",
            "GIT_AUTHOR_EMAIL": "test@example.invalid",
            "GIT_COMMITTER_NAME": "Statusline Test",
            "GIT_COMMITTER_EMAIL": "test@example.invalid",
        }
        self.repo = self.root / "repo"
        self.git("init", "-b", "main", self.repo)
        self.git("-C", self.repo, "commit", "--allow-empty", "-m", "fixture")

    def git(self, *args):
        return subprocess.run(
            ["git", "-c", "core.hooksPath=/dev/null", *map(str, args)],
            env=self.env,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()

    def render(self, cwd, *, workspace=True):
        payload = {"cwd": str(cwd), "model": {"display_name": "Test Model"}}
        if workspace:
            payload["cwd"] = str(self.repo)
            payload["workspace"] = {"current_dir": str(cwd)}
        result = subprocess.run(
            ["bash", str(MODULE / "files/statusline-command.sh")],
            input=json.dumps(payload),
            env=self.env,
            check=True,
            capture_output=True,
            text=True,
        )
        # Keep populated caches fresh regardless of runner scheduling delays.
        future = time.time() + 3600
        for path in self.cache.iterdir():
            os.utime(path, (future, future))
        return re.sub(r"\x1b\[[0-9;]*m", "", result.stdout).strip().split(" | ")

    def test_worktrees_keep_their_own_labels(self):
        for name in ("tab a", "tab b"):
            self.git("-C", self.repo, "worktree", "add", "-b", name.replace(" ", "-"), self.root / name)
        for name in ("tab a", "tab b", "tab a"):
            with self.subTest(worktree=name):
                self.assertEqual(
                    self.render(self.root / name),
                    [str(self.root / name), f"worktree({name})", "Test Model"],
                )

    def test_non_repository_does_not_inherit_or_clear_branch(self):
        plain = self.root / "plain"
        plain.mkdir()
        expected_repo = [str(self.repo), "main", "Test Model"]
        self.assertEqual(self.render(self.repo, workspace=False), expected_repo)
        self.assertEqual(self.render(plain, workspace=False), [str(plain), "Test Model"])
        self.assertEqual(self.render(self.repo, workspace=False), expected_repo)


if __name__ == "__main__":
    unittest.main()
