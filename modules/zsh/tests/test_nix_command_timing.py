import errno
import importlib.util
import json
import os
import pty
import select
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import termios
import time
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUNNER = ROOT / "nix-command-timing.py"
SHELL = ROOT / "nix-command-timing.zsh"
SPEC = importlib.util.spec_from_file_location("nix_command_timing", RUNNER)
timing = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(timing)


class StageTimingTest(unittest.TestCase):
    def test_home_manager_boundaries_partition_total(self):
        clock = timing.Timings("home-manager", 10)
        clock.line("Starting Home Manager activation", 12)
        clock.line("Activating checkFilesChanged", 13)
        clock.line("unrelated output", 16)
        clock.line("Activating linkGeneration", 18)
        rows = clock.finish(21)
        self.assertEqual(
            rows,
            [
                ("준비·평가·다운로드·빌드", 2),
                ("Activation 준비", 1),
                ("Activation: checkFilesChanged", 5),
                ("Activation: linkGeneration", 3),
                ("합계", 11),
            ],
        )
        self.assertEqual(sum(value for _, value in rows[:-1]), rows[-1][1])

    def test_ansi_and_repeated_marker_do_not_split_stage(self):
        clock = timing.Timings("home-manager", 0)
        clock.line("\x1b[32mActivating installPackages\x1b[0m", 2)
        clock.line("Activating installPackages", 4)
        self.assertEqual(clock.finish(6)[-2], ("Activation: installPackages", 4))

    def test_no_markers_reports_whole_elapsed_time(self):
        clock = timing.Timings("home-manager", 10)
        clock.line("a successful command with no activation", 11)
        self.assertEqual(clock.finish(14), [("전체 (단계 로그 없음)", 4), ("합계", 4)])

    def test_nixos_phases_and_home_manager_journal_are_separate(self):
        clock = timing.Timings("nixos-rebuild", 0)
        markers = [
            (1, "building the system configuration..."),
            (4, "stopping the following units: example.service"),
            (5, "activating the configuration..."),
            (7, "reloading user units for user..."),
            (8, "Starting Home Manager activation"),
            (9, "Activating setupClaudeCode"),
            (10, "restarting sysinit-reactivation.target"),
            (11, "starting the following units: home-manager-user.service"),
            (13, "Done. The new configuration is /nix/store/example"),
        ]
        for timestamp, marker in markers:
            clock.line(marker, timestamp)
        rows = clock.finish(14)
        self.assertEqual([elapsed for _, elapsed in rows], [1, 3, 1, 2, 6, 1, 14])
        self.assertEqual(sum(value for _, value in rows[:-1]), 14)
        self.assertFalse(any("setupClaudeCode" in label for label, _ in rows))

    def test_nixos_completion_message_without_stdout_store_path(self):
        self.assertEqual(
            timing.stage_for("nixos-rebuild", "Done. The new configuration is "),
            "마무리",
        )


class RunnerTest(unittest.TestCase):
    def command(self, child):
        return [
            sys.executable,
            str(RUNNER),
            "--tool",
            "home-manager",
            "--",
            sys.executable,
            "-c",
            child,
        ]

    def ready(self, process):
        readable, _, _ = select.select([process.stdout], [], [], 10)
        self.assertTrue(readable, "runner did not forward child output")
        self.assertEqual(os.read(process.stdout.fileno(), 5), b"READY")

    def terminal(self):
        master, slave = pty.openpty()
        self.addCleanup(os.close, master)
        self.addCleanup(os.close, slave)
        attributes = termios.tcgetattr(slave)
        attributes[1] &= ~termios.OPOST
        termios.tcsetattr(slave, termios.TCSANOW, attributes)
        return master, slave

    def read_terminal(self, master, marker=None):
        output = b""
        deadline = time.monotonic() + 10
        while marker is None or marker not in output:
            remaining = deadline - time.monotonic()
            self.assertGreater(remaining, 0, "runner terminal output timed out")
            readable, _, _ = select.select([master], [], [], remaining)
            self.assertTrue(readable, "runner did not forward terminal output")
            try:
                chunk = os.read(master, 65536)
            except OSError as error:
                if error.errno != errno.EIO:
                    raise
                break
            if not chunk:
                break
            output += chunk
        return output

    def test_preserves_each_output_terminal_and_redirect(self):
        for stdout_tty, stderr_tty in ((True, True), (True, False), (False, True)):
            with self.subTest(stdout_tty=stdout_tty, stderr_tty=stderr_tty):
                stdout_master, stdout_slave = self.terminal()
                stderr_master, stderr_slave = self.terminal()
                result = subprocess.run(
                    self.command(
                        "import os; "
                        "os.write(1,str(os.isatty(1)).encode()+b'\\n'); "
                        "os.write(2,str(os.isatty(2)).encode()+b'\\n')"
                    ),
                    stdout=stdout_slave if stdout_tty else subprocess.PIPE,
                    stderr=stderr_slave if stderr_tty else subprocess.PIPE,
                    timeout=10,
                    check=False,
                )
                stdout = (
                    self.read_terminal(stdout_master, b"\n")
                    if stdout_tty
                    else result.stdout
                )
                stderr = (
                    self.read_terminal(stderr_master, b"\n")
                    if stderr_tty
                    else result.stderr
                )
                self.assertEqual(result.returncode, 0)
                self.assertEqual(
                    stdout, str(stdout_tty).encode() + (b"\r\n" if stdout_tty else b"\n")
                )
                self.assertTrue(
                    stderr.startswith(
                        str(stderr_tty).encode() + (b"\r\n" if stderr_tty else b"\n")
                    )
                )

    def test_terminal_preserves_ansi_carriage_returns_and_newlines(self):
        stdout_master, stdout_slave = self.terminal()
        stderr_master, stderr_slave = self.terminal()
        payload = b"\x1b[32mprogress\x1b[0m\rnext\nlast\r\nEND"
        result = subprocess.run(
            self.command(
                f"import os; os.write(1,{payload!r}); os.write(2,{payload!r})"
            ),
            stdout=stdout_slave,
            stderr=stderr_slave,
            timeout=10,
            check=False,
        )
        # The test terminal is raw, as under sudo, so the runner must add the CR.
        displayed = payload.replace(b"\n", b"\r\n")
        self.assertEqual(result.returncode, 0)
        self.assertEqual(self.read_terminal(stdout_master, b"END"), displayed)
        self.assertTrue(self.read_terminal(stderr_master, b"END").startswith(displayed))

    def test_terminal_size_and_sigwinch_are_forwarded(self):
        master, slave = self.terminal()
        termios.tcsetwinsize(slave, (31, 97))
        process = subprocess.Popen(
            self.command(
                "import os,signal; "
                "report=lambda *_: os.write(1,(repr(tuple(os.get_terminal_size(1)))+'\\n').encode()); "
                "signal.signal(signal.SIGWINCH,report); report(); "
                "os.read(0,1)"
            ),
            stdin=subprocess.PIPE,
            stdout=slave,
            stderr=subprocess.PIPE,
            start_new_session=True,
        )
        try:
            self.assertEqual(self.read_terminal(master, b"\n"), b"(97, 31)\r\n")
            termios.tcsetwinsize(slave, (43, 121))
            process.send_signal(signal.SIGWINCH)
            self.assertEqual(self.read_terminal(master, b"\n"), b"(121, 43)\r\n")
            process.communicate(b"x", timeout=10)
            self.assertEqual(process.returncode, 0)
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
            process.communicate()

    def test_preserves_stdout_stderr_bytes_exit_status_and_stdin(self):
        payload = b"stdin\x00\xff\nsecond line"
        result = subprocess.run(
            self.command(
                "import os,sys; data=sys.stdin.buffer.read(); "
                "os.write(1,b'out\\x00\\xff\\r\\n'+data); "
                "os.write(2,b'err\\x00\\xfe\\rpartial'); sys.exit(7)"
            ),
            input=payload,
            capture_output=True,
            timeout=10,
            check=False,
        )
        self.assertEqual(result.returncode, 7)
        self.assertEqual(result.stdout, b"out\x00\xff\r\n" + payload)
        self.assertTrue(result.stderr.startswith(b"err\x00\xfe\rpartial"))
        self.assertIn("종료 코드: 7".encode(), result.stderr)
        self.assertIn("전체 (단계 로그 없음)".encode(), result.stderr)

    def test_streams_before_child_finishes_and_child_inherits_stdin(self):
        process = subprocess.Popen(
            self.command(
                "import os; os.write(1,b'READY'); os.write(2,b'ERR'); assert os.read(0,1)==b'x'; os.write(1,b'DONE')"
            ),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        try:
            self.ready(process)
            self.assertIsNone(process.poll())
            readable, _, _ = select.select([process.stderr], [], [], 10)
            self.assertTrue(readable, "runner did not forward child stderr")
            self.assertEqual(os.read(process.stderr.fileno(), 3), b"ERR")
            stdout, stderr = process.communicate(b"x", timeout=10)
            self.assertEqual(stdout, b"DONE")
            self.assertIn("종료 코드: 0".encode(), stderr)
            self.assertEqual(process.returncode, 0)
        finally:
            if process.poll() is None:
                process.kill()
            process.communicate()

    @unittest.skipUnless(os.name == "posix", "POSIX signals required")
    def test_sigterm_is_forwarded_and_reported(self):
        process = subprocess.Popen(
            self.command("import os,time; os.write(1,b'READY'); time.sleep(60)"),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
        )
        try:
            self.ready(process)
            process.send_signal(signal.SIGTERM)
            _, stderr = process.communicate(timeout=10)
            self.assertEqual(process.returncode, 143)
            self.assertIn("종료 코드: 143".encode(), stderr)
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
            process.communicate()

    @unittest.skipUnless(os.name == "posix", "POSIX signals required")
    def test_sigint_is_forwarded_and_reported(self):
        master, slave = self.terminal()
        process = subprocess.Popen(
            self.command("import os,time; os.write(1,b'READY'); time.sleep(60)"),
            stdout=slave,
            stderr=subprocess.PIPE,
            start_new_session=True,
        )
        try:
            self.assertEqual(self.read_terminal(master, b"READY"), b"READY")
            process.send_signal(signal.SIGINT)
            _, stderr = process.communicate(timeout=10)
            self.assertEqual(process.returncode, 130)
            self.assertIn("종료 코드: 130".encode(), stderr)
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
            process.communicate()


@unittest.skipUnless(shutil.which("zsh"), "zsh required")
class ShellIntegrationTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.folder = Path(temporary.name)
        self.record = self.folder / "calls.jsonl"
        for name in ("nix-command-timing", "home-manager", "nixos-rebuild", "sudo"):
            stub = self.folder / name
            stub.write_text(
                f"#!{sys.executable}\n"
                "import json,os,sys\n"
                "from pathlib import Path\n"
                "with open(os.environ['TIMING_TEST_RECORD'],'a') as output:\n"
                " output.write(json.dumps([Path(sys.argv[0]).name,*sys.argv[1:]])+'\\n')\n"
            )
            stub.chmod(0o755)

    def run_shell(self, body):
        result = subprocess.run(
            [
                shutil.which("zsh"),
                "-f",
                "-c",
                "source " + shlex.quote(str(SHELL)) + "\n" + body,
            ],
            env={
                **os.environ,
                "PATH": str(self.folder) + os.pathsep + os.environ["PATH"],
                "TIMING_TEST_RECORD": str(self.record),
            },
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        return [json.loads(line) for line in self.record.read_text().splitlines()]

    def test_direct_commands_and_aliases_preserve_arguments(self):
        calls = self.run_shell(
            "home-manager switch --flake 'path with spaces#host'\n"
            "nixos-rebuild switch --flake .#host\n"
            "alias hms='home-manager switch'\n"
            "eval \"hms --flake '.#alias'\"\n"
        )
        self.assertEqual(
            calls,
            [
                [
                    "nix-command-timing",
                    "--tool",
                    "home-manager",
                    "--",
                    "home-manager",
                    "switch",
                    "--flake",
                    "path with spaces#host",
                ],
                [
                    "nix-command-timing",
                    "--tool",
                    "nixos-rebuild",
                    "--",
                    "nixos-rebuild",
                    "switch",
                    "--flake",
                    ".#host",
                ],
                [
                    "nix-command-timing",
                    "--tool",
                    "home-manager",
                    "--",
                    "home-manager",
                    "switch",
                    "--flake",
                    ".#alias",
                ],
            ],
        )

    def test_sudo_rebuild_options_and_unrelated_command_forwarding(self):
        calls = self.run_shell(
            "sudo -u root -- nixos-rebuild switch --flake '.#host'\n"
            "sudo -E /run/current-system/sw/bin/nixos-rebuild test\n"
            "sudo -u 'other user' echo 'argument with spaces'\n"
        )
        self.assertEqual(
            calls,
            [
                [
                    "nix-command-timing",
                    "--tool",
                    "nixos-rebuild",
                    "--",
                    "sudo",
                    "-u",
                    "root",
                    "--",
                    "nixos-rebuild",
                    "switch",
                    "--flake",
                    ".#host",
                ],
                [
                    "nix-command-timing",
                    "--tool",
                    "nixos-rebuild",
                    "--",
                    "sudo",
                    "-E",
                    "/run/current-system/sw/bin/nixos-rebuild",
                    "test",
                ],
                ["sudo", "-u", "other user", "echo", "argument with spaces"],
            ],
        )


if __name__ == "__main__":
    unittest.main()
