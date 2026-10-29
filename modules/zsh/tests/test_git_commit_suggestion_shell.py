import os
import shlex
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

SHELL = Path(__file__).resolve().parents[1] / "git-commit-suggestion.zsh"
PLUGINS = sorted(
    Path("/nix/store").glob(
        "*-zsh-autosuggestions-*/share/zsh-autosuggestions/zsh-autosuggestions.zsh"
    )
)
PLUGIN = PLUGINS[-1] if PLUGINS else None
TRIGGERS = ("gc -m", "gc -m ", "gc -m '", 'gc -m "')


@unittest.skipUnless(shutil.which("zsh"), "zsh is not installed")
class CommitSuggestionShellTest(unittest.TestCase):
    def run_shell(self, body, message="fix: preserve literal arguments", plugin=False):
        setup = ""
        if plugin:
            if PLUGIN is None:
                self.skipTest("zsh-autosuggestions is not installed in /nix/store")
            setup = f"source {shlex.quote(str(PLUGIN))}\n"
        result = subprocess.run(
            [
                shutil.which("zsh"),
                "-f",
                "-c",
                setup
                + f"source {shlex.quote(str(SHELL))}\n"
                + 'git-commit-message() { print -r -- "$TEST_MESSAGE"; }\n'
                + body,
            ],
            env={**os.environ, "TEST_MESSAGE": message},
            capture_output=True,
            timeout=10,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        return result.stdout

    def test_triggers_preserve_prefix_and_quote_message_as_one_literal_argument(self):
        message = (
            "fix: keep $HOME $(print INJECTED) `print INJECTED` ' ! \\ \"; literal"
        )
        for trigger in TRIGGERS:
            with self.subTest(trigger=trigger):
                output = self.run_shell(
                    f"prefix={shlex.quote(trigger)}\n"
                    '_zsh_autosuggest_strategy_git_commit "$prefix"\n'
                    '[[ "$suggestion" == "$prefix"* ]] || exit 10\n'
                    'gc() { printf "%s\\0" "$@"; }\n'
                    'eval "$suggestion"\n',
                    message=message,
                )
                self.assertEqual(output.split(b"\0"), [b"-m", message.encode(), b""])

    def test_unrelated_and_already_started_messages_do_not_call_helper(self):
        for buffer in (
            "g",
            "gc",
            "gc ",
            "gc -",
            "gc -ma",
            "gc -m  ",
            "gc -m message",
            "gc -m 'message",
            'gc -m "message',
            "git commit -m",
            "echo gc -m",
            "gc -am",
            "gc -m\n",
        ):
            with self.subTest(buffer=buffer):
                self.assertEqual(
                    self.run_shell(
                        "git-commit-message() { print CALLED; return 1; }\n"
                        f"_zsh_autosuggest_strategy_git_commit {shlex.quote(buffer)}\n"
                        'print -rn -- "$suggestion"\n'
                    ),
                    b"",
                )

    def test_helper_errors_and_empty_output_do_not_suggest(self):
        for helper in (
            "return 1",
            "print partial; return 1",
            "return 0",
        ):
            with self.subTest(helper=helper):
                self.assertEqual(
                    self.run_shell(
                        f"git-commit-message() {{ {helper}; }}\n"
                        "_zsh_autosuggest_strategy_git_commit 'gc -m'\n"
                        'print -rn -- "$suggestion"\n'
                    ),
                    b"",
                )

    def test_plugin_history_keeps_normal_commands_and_excludes_stale_commits(self):
        with tempfile.NamedTemporaryFile(mode="w") as history:
            # The last history event is the current command and is not a candidate.
            history.write("gc -m 'stale message'\necho ordinary history\ncurrent\n")
            history.flush()
            output = self.run_shell(
                f"HISTSIZE=100\nfc -R {shlex.quote(history.name)}\n"
                "git-commit-message() { return 1; }\n"
                "for prefix in g gc 'gc ' 'gc -' 'gc -m'; do\n"
                "  unset suggestion\n"
                '  _zsh_autosuggest_fetch_suggestion "$prefix"\n'
                '  [[ -z "$suggestion" ]] || exit 20\n'
                "done\n"
                "_zsh_autosuggest_fetch_suggestion 'echo ord'\n"
                'print -rn -- "$suggestion"\n',
                plugin=True,
            )
        self.assertEqual(output, b"echo ordinary history")

    def test_plugin_accept_puts_ghost_text_into_buffer_without_executing(self):
        output = self.run_shell(
            "BUFFER='gc -m'\n"
            "CURSOR=$#BUFFER\n"
            "KEYMAP=emacs\n"
            ' _zsh_autosuggest_fetch_suggestion "$BUFFER"\n'
            'expected="$suggestion"\n'
            '_zsh_autosuggest_suggest "$suggestion"\n'
            '[[ -n "$POSTDISPLAY" && "$BUFFER" == "gc -m" ]] || exit 30\n'
            "gc() { print EXECUTED; }\n"
            "_zsh_autosuggest_invoke_original_widget() { return 0; }\n"
            "_zsh_autosuggest_accept forward-char\n"
            '[[ "$BUFFER" == "$expected" && -z "$POSTDISPLAY" ]] || exit 31\n'
            "(( CURSOR == $#BUFFER )) || exit 32\n"
            "print ACCEPTED\n",
            plugin=True,
        )
        self.assertEqual(output, b"ACCEPTED\n")


if __name__ == "__main__":
    unittest.main()
