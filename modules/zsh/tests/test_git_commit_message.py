import importlib.util
import io
import json
import os
import subprocess
import tempfile
import unittest
import urllib.error
from pathlib import Path
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    "git_commit_message",
    Path(__file__).resolve().parents[1]
    / "plugins/git-commit-suggestions/git-commit-message.py",
)
message = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(message)


class ContextTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        previous = Path.cwd()
        self.addCleanup(os.chdir, previous)
        os.chdir(self.root)
        self.git("init", "-q")
        self.fake_home = self.root / "home"
        self.fake_home.mkdir()
        home = patch.object(Path, "home", return_value=self.fake_home)
        home.start()
        self.addCleanup(home.stop)

    def git(self, *args):
        return subprocess.run(
            ["git", *args], check=True, capture_output=True, text=True
        ).stdout

    def test_staged_content_and_root_rules_from_subdirectory_on_unborn_branch(self):
        (self.fake_home / ".codex").mkdir()
        (self.fake_home / ".codex/AGENTS.md").write_text("Use concise subjects.")
        (self.root / "AGENTS.md").write_text("Write subjects in Korean.")
        (self.root / "CLAUDE.md").write_text("Use Conventional Commits.")
        (self.root / "code").write_text("staged content\n")
        self.git("add", "code")
        (self.root / "code").write_text("unstaged content\n")
        (self.root / "untracked").write_text("private untracked content\n")
        before = self.git("diff", "--cached")
        os.chdir(self.fake_home)
        data = message.context()
        self.assertIn("staged content", data["staged_diff"])
        self.assertNotIn("unstaged content", data["staged_diff"])
        self.assertNotIn("private untracked content", json.dumps(data))
        self.assertEqual(data["recent_subjects"], "")
        self.assertEqual(
            data["rules"],
            {
                "global AGENTS.md": "Use concise subjects.",
                "repository AGENTS.md": "Write subjects in Korean.",
                "repository CLAUDE.md": "Use Conventional Commits.",
            },
        )
        self.assertEqual(self.git("diff", "--cached"), before)

    def test_history_and_missing_optional_rules(self):
        self.git(
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "--allow-empty",
            "-qm",
            "fix: previous style",
        )
        (self.root / "code").write_text("new content\n")
        self.git("add", "code")
        data = message.context()
        self.assertEqual(data["rules"], {})
        self.assertEqual(data["recent_subjects"], "fix: previous style\n")

    def test_empty_index_does_not_call_api(self):
        with patch.object(message, "generate") as generate:
            self.assertEqual(message.main(), 0)
        generate.assert_not_called()

    def test_large_diff_is_rejected_before_api(self):
        (self.root / "large").write_text("x" * 64001)
        self.git("add", "large")
        with self.assertRaisesRegex(ValueError, "staged diff exceeds"):
            message.context()


class CompletionTest(unittest.TestCase):
    def response(self, content="feat: add commit suggestions", finish="stop"):
        return io.BytesIO(
            json.dumps(
                {
                    "choices": [
                        {
                            "message": {"content": content},
                            "finish_reason": finish,
                        }
                    ]
                }
            ).encode()
        )

    @patch.dict(os.environ, {"OPENAI_API_KEY": "test-key"})
    def test_request_contract_and_literal_subject(self):
        subject = "fix: preserve '$HOME', `code`, and $(commands)!"
        with patch.object(
            message.urllib.request, "urlopen", return_value=self.response(subject)
        ) as send:
            self.assertEqual(message.generate({"staged_diff": "a diff"}), subject)
        request = send.call_args.args[0]
        payload = json.loads(request.data)
        self.assertEqual(request.full_url, "https://api.openai.com/v1/chat/completions")
        self.assertEqual(payload["model"], "gpt-6.1-terra")
        self.assertFalse(payload["store"])
        self.assertEqual(
            json.loads(payload["messages"][1]["content"])["staged_diff"], "a diff"
        )
        self.assertEqual(send.call_args.kwargs["timeout"], 8)

    @patch.dict(os.environ, {"OPENAI_API_KEY": "test-key"})
    def test_rejects_empty_multiline_control_or_incomplete_output(self):
        for content, finish in (
            ("", "stop"),
            (None, "stop"),
            ("a\nb", "stop"),
            ("a\x1bb", "stop"),
            ("partial", "length"),
        ):
            with (
                self.subTest(content=content, finish=finish),
                patch.object(
                    message.urllib.request,
                    "urlopen",
                    return_value=self.response(content, finish),
                ),
                self.assertRaises(ValueError),
            ):
                message.generate({})

    @patch.dict(os.environ, {"OPENAI_API_KEY": "test-key"})
    def test_http_error_does_not_expose_response_body(self):
        error = urllib.error.HTTPError("url", 401, "denied", {}, io.BytesIO(b"private"))
        with (
            patch.object(message.urllib.request, "urlopen", side_effect=error),
            self.assertRaisesRegex(ValueError, "^OpenAI returned HTTP 401$"),
        ):
            message.generate({})


if __name__ == "__main__":
    unittest.main()
