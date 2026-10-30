#!/usr/bin/env python3
from __future__ import annotations

import os
import subprocess
import tempfile
import time
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parent
FILES = ROOT / "files"


class AgentStatusTest(unittest.TestCase):
    def test_status_set_targets_originating_tmux_pane(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            cache_home = root / "cache"
            bin_dir = root / "bin"
            tmux_log = root / "tmux.log"
            fake_tmux = bin_dir / "tmux"
            bin_dir.mkdir()
            fake_tmux.write_text(
                "#!/usr/bin/env bash\n"
                "printf '%s\\n' \"$*\" >> \"$TMUX_TEST_LOG\"\n"
                "if [ \"${1:-}\" = display-message ]; then printf '@42\\n'; fi\n"
            )
            fake_tmux.chmod(0o755)

            env = os.environ.copy()
            env.update(
                {
                    "PATH": f"{bin_dir}:{env['PATH']}",
                    "TMUX": "/tmp/tmux-test,1,0",
                    "TMUX_PANE": "%7",
                    "TMUX_TEST_LOG": str(tmux_log),
                    "XDG_CACHE_HOME": str(cache_home),
                }
            )

            result = subprocess.run(
                ["bash", str(FILES / "agent-status-set.sh"), "claude", "running"],
                env=env,
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertEqual(0, result.returncode)
            self.assertTrue((cache_home / "tmux-agent-status" / "@42").is_file())
            self.assertEqual(
                "display-message -p -t %7 #{window_id}",
                tmux_log.read_text().splitlines()[0],
            )

    def test_status_set_and_window_status_accept_tmux_window_id(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            cache_home = Path(tmp) / "cache"
            env = os.environ.copy()
            env["XDG_CACHE_HOME"] = str(cache_home)

            set_result = subprocess.run(
                [
                    "bash",
                    str(FILES / "agent-status-set.sh"),
                    "codex",
                    "running",
                    "@12",
                ],
                env=env,
                check=False,
                capture_output=True,
                text=True,
            )
            window_result = subprocess.run(
                [
                    "bash",
                    str(FILES / "agent-status-window.sh"),
                    "@12",
                ],
                env=env,
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertEqual(0, set_result.returncode)
            self.assertEqual(0, window_result.returncode)
            self.assertEqual("#[fg=blue]●#[default]", window_result.stdout)

    def test_status_set_rejects_window_id_path_traversal(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            cache_home = Path(tmp) / "cache"
            env = os.environ.copy()
            env["XDG_CACHE_HOME"] = str(cache_home)

            result = subprocess.run(
                [
                    "bash",
                    str(FILES / "agent-status-set.sh"),
                    "codex",
                    "running",
                    "../outside",
                ],
                env=env,
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertEqual(2, result.returncode)
            self.assertFalse((cache_home / "outside").exists())

    def test_window_status_ignores_invalid_window_id_path_traversal(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            cache_home = Path(tmp) / "cache"
            status_dir = cache_home / "tmux-agent-status"
            status_dir.mkdir(parents=True)
            outside = cache_home / "outside"
            outside.write_text(f"failed\tcodex\t{int(time.time())}\tcodex\n")

            env = os.environ.copy()
            env["XDG_CACHE_HOME"] = str(cache_home)

            result = subprocess.run(
                [
                    "bash",
                    str(FILES / "agent-status-window.sh"),
                    "../outside",
                ],
                env=env,
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertEqual(0, result.returncode)
            self.assertEqual("", result.stdout)


if __name__ == "__main__":
    unittest.main()
