"""Exercise activation and login through the launcher CLI with a fake mount command."""

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest


RUNNER = Path(__file__).resolve().parents[1] / "runner.py"


class RunnerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.state = self.root / "state"
        self.wrapper = self.root / "agenix-launchd-wrapper"
        self.source = self.root / "wrapper-source"
        self.source.write_text("#!/bin/sh\nexit 0\n")
        self.calls = self.root / "calls"
        self.secrets = self.root / "secrets"
        self.mount = self.root / "mount"
        self.mount.write_text(
            f"#!{sys.executable}\n"
            "import sys\n"
            "from pathlib import Path\n"
            f"with Path({str(self.calls)!r}).open('a') as output: output.write('mount\\n')\n"
            f"failure = Path({str(self.root / 'fail-next')!r})\n"
            "if failure.exists():\n"
            "    failure.unlink()\n"
            "    sys.exit(9)\n"
            f"Path({str(self.secrets)!r}).mkdir(exist_ok=True)\n"
        )
        self.mount.chmod(0o755)

    def command(self, mode="activate", mount=None):
        args = [sys.executable, str(RUNNER), mode, "--state-dir", str(self.state)]
        if mode == "activate":
            args += [
                "--mount-script",
                str(mount or self.mount),
                "--wrapper-source",
                str(self.source),
                "--wrapper-path",
                str(self.wrapper),
                "--secrets-dir",
                str(self.secrets),
            ]
        return args

    def run_command(self, mode="activate", mount=None):
        return subprocess.run(self.command(mode, mount), capture_output=True, text=True)

    def test_unchanged_activation_skips_mount_but_login_runs(self):
        first = self.run_command()
        self.assertEqual(first.returncode, 0, first.stderr)
        self.assertEqual(self.wrapper.read_bytes(), self.source.read_bytes())
        self.assertTrue(os.access(self.wrapper, os.X_OK))
        self.assertEqual(self.run_command().returncode, 0)
        self.assertEqual(self.calls.read_text(), "mount\n")
        login = self.run_command("login")
        self.assertEqual(login.returncode, 0, login.stderr)
        self.assertEqual(self.calls.read_text(), "mount\nmount\n")

    def test_activation_retries_failed_login_of_same_script(self):
        self.assertEqual(self.run_command().returncode, 0)
        (self.root / "fail-next").touch()
        failed = self.run_command("login")
        self.assertEqual(failed.returncode, 9)
        self.assertIn("mount command failed", failed.stderr)
        self.assertEqual(self.run_command().returncode, 0)
        self.assertEqual(self.calls.read_text(), "mount\nmount\nmount\n")

    def test_activation_repairs_nonexecutable_wrapper(self):
        self.assertEqual(self.run_command().returncode, 0)
        self.wrapper.chmod(0o600)
        result = self.run_command()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(os.access(self.wrapper, os.X_OK))

    def test_generation_update_and_rollback_select_login_command(self):
        replacement = self.root / "replacement"
        replacement.write_text(
            self.mount.read_text().replace("mount\\n", "replacement\\n")
        )
        replacement.chmod(0o755)
        for mode, mount in (
            ("activate", self.mount),
            ("activate", replacement),
            ("login", None),
            ("activate", self.mount),
            ("login", None),
        ):
            result = self.run_command(mode, mount)
            self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.calls.read_text(), "mount\nreplacement\nreplacement\nmount\nmount\n"
        )

    def test_activation_recovers_missing_secret_directory(self):
        self.assertEqual(self.run_command().returncode, 0)
        self.secrets.rmdir()
        result = self.run_command()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(self.secrets.is_dir())
        self.assertEqual(self.calls.read_text(), "mount\nmount\n")

    def test_concurrent_activations_do_not_overlap_or_mount_twice(self):
        self.check_concurrent_mounts("activate", "mount\n")

    def test_login_waits_for_activation_before_mounting(self):
        self.check_concurrent_mounts("login", "mount\nmount\n")

    def check_concurrent_mounts(self, second_mode, expected_calls):
        started = self.root / "started"
        release = self.root / "release"
        self.mount.write_text(
            f"#!{sys.executable}\n"
            "import time\nfrom pathlib import Path\n"
            f"with Path({str(started)!r}).open('x'): pass\n"
            f"while not Path({str(release)!r}).exists(): time.sleep(0.01)\n"
            f"with Path({str(self.calls)!r}).open('a') as output: output.write('mount\\n')\n"
            f"Path({str(self.secrets)!r}).mkdir(exist_ok=True)\n"
            f"Path({str(started)!r}).unlink()\n"
        )
        first = subprocess.Popen(
            self.command(), stdout=subprocess.PIPE, stderr=subprocess.PIPE
        )
        second = None
        try:
            deadline = time.monotonic() + 5
            while not started.exists() and time.monotonic() < deadline:
                time.sleep(0.01)
            self.assertTrue(started.exists(), "first mount did not start")
            second = subprocess.Popen(
                self.command(second_mode),
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            with self.assertRaises(subprocess.TimeoutExpired):
                second.communicate(timeout=0.5)
            release.touch()
            self.assertEqual(first.communicate(timeout=5)[1], b"")
            self.assertEqual(second.communicate(timeout=5)[1], b"")
            self.assertEqual(first.returncode, 0)
            self.assertEqual(second.returncode, 0)
            self.assertEqual(self.calls.read_text(), expected_calls)
        finally:
            release.touch()
            for process in (first, second):
                if process is not None:
                    if process.poll() is None:
                        process.terminate()
                    process.communicate(timeout=5)


if __name__ == "__main__":
    unittest.main()
