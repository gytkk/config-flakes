from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch


FILES_DIR = Path(__file__).resolve().parents[1] / "files"
sys.path.insert(0, str(FILES_DIR))

from agent_session_provider import get_provider  # noqa: E402
from agent_session_upload_worker import Recorder  # noqa: E402


class UploadWorkerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.recorder = Recorder(get_provider("codex"), {"HOME": str(self.root)})
        self.lock_path = self.recorder.lock_dir / "test.lock"

    def old_lock(self, pid):
        self.lock_path.mkdir()
        (self.lock_path / "pid").write_text(str(pid))
        (self.lock_path / "created_epoch").write_text(str(int(time.time()) - 301))

    def test_live_owner_never_expires_by_age(self):
        self.old_lock(os.getpid())
        self.assertFalse(self.recorder._lock_is_stale(self.lock_path))

    def test_permission_denied_owner_is_treated_as_alive(self):
        self.old_lock(os.getpid())
        with patch("os.kill", side_effect=PermissionError):
            self.assertFalse(self.recorder._lock_is_stale(self.lock_path))

    def test_waiter_does_not_steal_old_live_lock(self):
        self.old_lock(os.getpid())
        with patch("time.monotonic", side_effect=[0, 16]):
            with self.assertRaises(TimeoutError):
                with self.recorder.lock(self.lock_path):
                    self.fail("acquired a live worker's lock")

    def test_release_preserves_replacement_owner(self):
        with self.recorder.lock(self.lock_path):
            shutil.rmtree(self.lock_path)
            self.lock_path.mkdir()
            (self.lock_path / "pid").write_text(str(os.getpid()))
            (self.lock_path / "token").write_text("replacement owner")
        self.assertEqual((self.lock_path / "token").read_text(), "replacement owner")

    def test_dead_owner_is_reclaimed(self):
        self.old_lock(99999999)
        with patch("os.kill", side_effect=ProcessLookupError):
            with self.recorder.lock(self.lock_path):
                self.assertEqual(int((self.lock_path / "pid").read_text()), os.getpid())
        self.assertFalse(self.lock_path.exists())

    def test_failed_initialization_removes_own_directory(self):
        with patch.object(Path, "write_text", side_effect=OSError("disk failure")):
            with self.assertRaisesRegex(OSError, "disk failure"):
                with self.recorder.lock(self.lock_path):
                    self.fail("lock initialization should fail")
        self.assertFalse(self.lock_path.exists())
        with self.recorder.lock(self.lock_path):
            self.assertTrue(self.lock_path.is_dir())

    def test_body_failure_releases_own_lock(self):
        with self.assertRaisesRegex(RuntimeError, "worker failed"):
            with self.recorder.lock(self.lock_path):
                raise RuntimeError("worker failed")
        self.assertFalse(self.lock_path.exists())

    def test_abandoned_initialization_expires_but_recent_one_does_not(self):
        self.lock_path.mkdir()
        self.assertFalse(self.recorder._lock_is_stale(self.lock_path))
        os.utime(self.lock_path, (time.time() - 301, time.time() - 301))
        self.assertTrue(self.recorder._lock_is_stale(self.lock_path))

    def test_contenders_reclaim_dead_lock_without_overlapping(self):
        self.old_lock(99999999)
        counter = self.root / "counter"
        counter.write_text("0")

        def increment(_):
            for _ in range(5):
                with self.recorder.lock(self.lock_path):
                    value = int(counter.read_text())
                    time.sleep(0.001)
                    counter.write_text(str(value + 1))

        with ThreadPoolExecutor(max_workers=4) as pool:
            list(pool.map(increment, range(4)))
        self.assertEqual(counter.read_text(), "20")
        self.assertFalse(self.lock_path.exists())

    def test_upload_steps_share_one_deadline(self):
        manifest = {
            "run_id": "run_test",
            "transcript_fingerprint": "fingerprint",
            "redaction_status": "succeeded",
            "privacy_check_status": "succeeded",
        }
        with (
            patch("time.monotonic", side_effect=[100, 110, 160, 250, 390]),
            patch.object(self.recorder, "run_checked") as run,
        ):
            self.recorder.upload_remote(
                self.root / "transcript",
                self.root / "meta",
                self.root / "remote",
                manifest,
            )
        self.assertEqual(
            [call.kwargs["timeout"] for call in run.call_args_list], [290, 240, 150, 10]
        )

    def test_every_upload_entrypoint_rejects_unsanitized_snapshots(self):
        transcript = self.root / "transcript"
        meta = self.root / "meta"
        target = self.root / "remote"
        transcript.write_text('{"content":"sk-test-secret-value-that-is-long"}\n')
        meta.write_text("{}\n")
        for redaction, privacy in (
            ("failed", "failed"),
            ("failed", "succeeded"),
            ("succeeded", "failed"),
            ("pending", "pending"),
            (None, None),
        ):
            with self.subTest(redaction=redaction, privacy=privacy):
                manifest = {
                    "redaction_status": redaction,
                    "privacy_check_status": privacy,
                }
                with (
                    patch.object(self.recorder, "run_checked") as transport,
                    patch("shutil.copyfile") as copy,
                ):
                    self.assertFalse(self.recorder.upload_snapshot(manifest, transcript))
                    with self.assertRaises(ValueError):
                        self.recorder.queue_snapshot(manifest, transcript)
                    for upload in (
                        self.recorder.upload_local,
                        self.recorder.upload_remote,
                    ):
                        with self.assertRaises(ValueError):
                            upload(transcript, meta, target, manifest)
                transport.assert_not_called()
                copy.assert_not_called()
                self.assertFalse(target.exists())
        self.assertEqual(list(self.recorder.queue_dir.iterdir()), [])
        self.assertEqual(list(self.recorder.tmp_dir.iterdir()), [])
        self.assertEqual(list(self.recorder.session_state_dir.iterdir()), [])

    def test_expired_budget_does_not_start_another_transport(self):
        with patch("subprocess.Popen") as popen:
            with self.assertRaises(subprocess.TimeoutExpired):
                self.recorder.run_checked("ssh", timeout=0)
        popen.assert_not_called()

    def test_transport_preserves_error_output(self):
        with self.assertRaises(subprocess.CalledProcessError) as caught:
            self.recorder.run_checked(
                sys.executable,
                "-c",
                "import sys; print('transport failed', file=sys.stderr); sys.exit(7)",
                timeout=5,
            )
        self.assertEqual(caught.exception.returncode, 7)
        self.assertIn("transport failed", caught.exception.stderr)

    def test_timeout_stops_transport_child_before_releasing_lock(self):
        ready = self.root / "child.pid"
        late_write = self.root / "late-write"
        script = self.root / "transport.py"
        script.write_text("""import os, signal, subprocess, sys, time
from pathlib import Path
if sys.argv[1] == "child":
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    Path(sys.argv[2]).write_text(str(os.getpid()))
    time.sleep(0.5)
    Path(sys.argv[3]).write_text("late write")
else:
    subprocess.Popen([sys.executable, __file__, "child", *sys.argv[2:]])
    if sys.argv[1] == "wait":
        time.sleep(10)
""")
        communicate = subprocess.Popen.communicate
        processes = []

        def communicate_when_ready(process, *args, **kwargs):
            processes.append(process)
            deadline = time.monotonic() + 5
            while not ready.exists() and time.monotonic() < deadline:
                time.sleep(0.01)
            self.assertTrue(ready.exists(), "transport child did not start")
            return communicate(process, *args, **kwargs)

        for mode in ("wait", "exit"):
            with self.subTest(mode=mode):
                ready.unlink(missing_ok=True)
                try:
                    with patch.object(
                        subprocess.Popen, "communicate", communicate_when_ready
                    ):
                        with self.assertRaises(subprocess.TimeoutExpired):
                            with self.recorder.lock(self.lock_path):
                                self.recorder.run_checked(
                                    sys.executable,
                                    script,
                                    mode,
                                    ready,
                                    late_write,
                                    timeout=0.1,
                                )
                    self.assertFalse(self.lock_path.exists())
                    self.assertIsNotNone(processes[-1].returncode)
                    time.sleep(0.6)
                    self.assertFalse(late_write.exists())
                finally:
                    if ready.exists():
                        try:
                            os.kill(int(ready.read_text()), signal.SIGKILL)
                        except ProcessLookupError:
                            pass


if __name__ == "__main__":
    unittest.main()
