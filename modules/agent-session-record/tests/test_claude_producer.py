import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "files"))
from agent_session_hook import claude_no_persistence


class ClaudeProducerTest(unittest.TestCase):
    @unittest.skipUnless(sys.platform == "linux", "Linux procfs producer probe")
    def test_only_first_verified_producer_and_unambiguous_option(self):
        with tempfile.TemporaryDirectory() as directory:
            proc = Path(directory)
            exe = "/nix/store/" + "a" * 32 + "-claude-code-2.1.330/bin/.claude-wrapped"

            def process(pid, parent, executable, args):
                path = proc / str(pid)
                path.mkdir(exist_ok=True)
                (path / "exe").unlink(missing_ok=True)
                (path / "exe").symlink_to(executable)
                (path / "status").write_text(f"PPid:\t{parent}\n")
                (path / "cmdline").write_bytes(
                    b"\0".join(x.encode() for x in args) + b"\0"
                )

            flag = "--no-session-persistence"
            process(11, 1, exe, ["claude", "-p", flag])
            for args, expected in [
                (["-p", flag, "prompt"], True),
                (["-p"], False),
                (["--", flag], False),
                (["--system-prompt", flag], False),
                (["prompt " + flag], False),
            ]:
                process(12, 11, exe, ["claude", *args])
                self.assertEqual(claude_no_persistence(12, proc), expected, args)
            process(12, 11, "/tmp/.claude-wrapped", ["claude", flag])
            self.assertFalse(claude_no_persistence(12, proc))
            process(
                12,
                11,
                "/bin/bash",
                [
                    "bash",
                    "-c",
                    "~/.local/bin/agent-session-record hook claude session-end",
                ],
            )
            self.assertTrue(claude_no_persistence(12, proc))
