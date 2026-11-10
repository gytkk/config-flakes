import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
import unittest


class WithOpenAITest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        root = Path(self.directory.name)
        self.key_file = root / "key with spaces"
        self.script = root / "with-openai.sh"
        source = Path(__file__).with_name("with-openai.sh").read_text()
        self.script.write_text(
            source.replace("@secretPath@", shlex.quote(str(self.key_file)))
        )

    def run_wrapper(self, *args, trace=False, runtime_directory=None):
        environment = os.environ.copy()
        environment["OPENAI_API_KEY"] = "stale-inherited-key"
        if runtime_directory is not None:
            environment["XDG_RUNTIME_DIR"] = str(runtime_directory)
        return subprocess.run(
            ["bash", *(["-x"] if trace else []), str(self.script), *args],
            env=environment,
            capture_output=True,
            text=True,
        )

    def test_key_is_only_in_child_environment_and_arguments_survive(self):
        key = "synthetic-key-$(must-not-run);'\""
        self.key_file.write_text(key + "\n")
        result = self.run_wrapper(
            sys.executable,
            "-c",
            "import os,sys; from pathlib import Path; "
            "assert os.environ['OPENAI_API_KEY'] == Path(sys.argv[1]).read_text().rstrip('\\n'); "
            "assert sys.argv[2:] == ['two words', '*', '']; print('ok')",
            str(self.key_file),
            "two words",
            "*",
            "",
            trace=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "ok\n")
        self.assertNotIn(key, result.stdout + result.stderr)

    def test_missing_secret_stops_before_command(self):
        result = self.run_wrapper("bash", "-c", "echo should-not-run")
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, "")
        self.assertIn("activate agenix", result.stderr)

    def test_nix_rendered_runtime_secret_path(self):
        repository = Path(__file__).resolve().parents[2]
        expression = r"""
          let
            flake = builtins.getFlake @repository@;
            module = import (builtins.toPath (@repository@ + "/modules/openai-api/default.nix")) {
              lib = flake.inputs.nixpkgs.lib;
              config = {
                modules.openaiApi.enable = true;
                age.secrets.openai-api-key.path = "\${XDG_RUNTIME_DIR}/agenix/openai-api-key";
              };
              pkgs = {
                coreutils = null;
                writeShellApplication = args: args;
              };
            };
          in (builtins.head module.config.content.home.packages).text
        """.replace("@repository@", json.dumps(str(repository)))
        rendered = subprocess.run(
            ["nix", "eval", "--offline", "--impure", "--raw", "--expr", expression],
            capture_output=True,
            text=True,
        )
        self.assertEqual(rendered.returncode, 0, rendered.stderr)
        self.script.write_text(rendered.stdout)
        runtime_directory = Path(self.directory.name) / "runtime with spaces"
        self.key_file = runtime_directory / "agenix" / "openai-api-key"
        self.key_file.parent.mkdir(parents=True)
        self.key_file.write_text("synthetic-runtime-key\n")
        result = self.run_wrapper(
            "bash",
            "-c",
            'test "$OPENAI_API_KEY" = synthetic-runtime-key',
            runtime_directory=runtime_directory,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_invalid_secret_stops_without_disclosing_value(self):
        for value in (
            "",
            "\n",
            "synthetic-first\nsynthetic-second",
            "synthetic-key\r\n",
        ):
            with self.subTest(value=value):
                self.key_file.write_text(value)
                result = self.run_wrapper("bash", "-c", "echo should-not-run")
                self.assertEqual(result.returncode, 1)
                self.assertEqual(result.stdout, "")
                self.assertIn("single nonempty API key", result.stderr)
                self.assertNotIn("synthetic-", result.stderr)

    def test_read_failure_stops_before_command(self):
        self.key_file.mkdir()
        result = self.run_wrapper("bash", "-c", "echo should-not-run")
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, "")
        self.assertIn("failed to read", result.stderr)

    def test_child_status_and_streams_are_preserved(self):
        self.key_file.write_text("synthetic-key")
        result = self.run_wrapper("bash", "-c", "echo out; echo err >&2; exit 42")
        self.assertEqual(result.returncode, 42)
        self.assertEqual(result.stdout, "out\n")
        self.assertEqual(result.stderr, "err\n")

    def test_usage_does_not_need_secret(self):
        self.assertEqual(self.run_wrapper().returncode, 2)
        result = self.run_wrapper("--help")
        self.assertEqual(result.returncode, 0)
        self.assertIn("Usage:", result.stdout)


if __name__ == "__main__":
    unittest.main()
