"""Evaluate setup shell generation without network access or host activation."""
import json
from pathlib import Path
import subprocess
import unittest

MODULE = Path(__file__).resolve().parents[1] / "default.nix"


def render_setup(module, mumu):
    expression = '''
      let
        lib = {
          mkIf = condition: value: if condition then value else {};
          optional = condition: value: if condition then [value] else [];
          hm.dag.entryAfter = after: data: { inherit after data; };
          concatMapStringsSep = separator: f: values:
            builtins.concatStringsSep separator (map f values);
          makeBinPath = packages: builtins.concatStringsSep ":" (map (p: "${p}/bin") packages);
          escapeShellArg = value: "'" + builtins.replaceStrings ["'"] ["'\\''"] value + "'";
        };
        pkgs = builtins.listToAttrs (map (name: { inherit name; value = "/fixture/${name}"; })
          ["claude-code" "coreutils" "ripgrep" "jq" "python3" "git" "openssh" "which"])
          // { writeText = name: text: "/fixture/${name}"; };
        result = import MODULE {
          inherit lib pkgs;
          config.modules.claude = { enable = true; mumu.enable = MUMU; };
          flakeDirectory = "/checkout with spaces";
        };
      in result.config.home.activation.setupClaudeCode
    '''.replace("MODULE", str(module)).replace("MUMU", "true" if mumu else "false")
    result = subprocess.run(
        ["nix", "eval", "--impure", "--json", "--expr", expression],
        capture_output=True, text=True,
    )
    if result.returncode:
        raise AssertionError(result.stderr)
    return json.loads(result.stdout)


class ActivationGenerationTests(unittest.TestCase):
    def test_setup_preserves_ordering_shell_syntax_and_profile_selection(self):
        for mumu in [False, True]:
            with self.subTest(mumu=mumu):
                activation = render_setup(MODULE, mumu)
                self.assertEqual(activation["after"], ["writeBoundary"])
                script = activation["data"]
                result = subprocess.run(["bash", "-n"], input=script, text=True, capture_output=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual("devsisters/mumu" in script, mumu)
                self.assertIn("timeout --foreground", script)
                self.assertIn("Restored settings.json symlink", script)
                self.assertIn("'/checkout with spaces/config/modules/claude/marketplace'", script)
