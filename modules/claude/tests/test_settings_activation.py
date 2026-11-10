"""Keep host-local marketplace registration out of shared Claude settings."""

import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

MODULE = Path(__file__).resolve().parents[1]


class SettingsActivationTests(unittest.TestCase):
    def activation(self, checkout, binaries):
        expression = """
          let
            lib = {
              mkIf = condition: value: if condition then value else {};
              optional = condition: value: if condition then [value] else [];
              concatMapStringsSep = sep: f: xs: builtins.concatStringsSep sep (map f xs);
              escapeShellArg = value: "'" + value + "'";
              makeBinPath = packages: BINARY_PATH + "/bin";
              hm.dag.entryAfter = after: data: { inherit after data; };
            };
            result = import MODULE_PATH {
              inherit lib;
              config.modules.claude = { enable = true; mumu.enable = false; };
              flakeDirectory = CHECKOUT_PATH;
              pkgs = {
                claude-code = BINARY_PATH;
                coreutils = BINARY_PATH;
                jq = BINARY_PATH;
                ripgrep = BINARY_PATH;
                python3 = BINARY_PATH;
                writeText = name: text: "/unused-plugin-list.json";
              };
            };
          in result.config.home.activation.setupClaudeCode.data
        """
        for key, value in {
            "MODULE_PATH": str(MODULE / "default.nix"),
            "BINARY_PATH": json.dumps(str(binaries)),
            "CHECKOUT_PATH": json.dumps(str(checkout)),
        }.items():
            expression = expression.replace(key, value)
        result = subprocess.run(
            ["nix", "eval", "--impure", "--json", "--expr", expression],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        return json.loads(result.stdout)

    def test_shared_settings_do_not_declare_local_marketplace(self):
        settings = json.loads((MODULE / "files/settings.json").read_text())
        self.assertNotIn("gytkk", settings["extraKnownMarketplaces"])

    def test_marketplace_registration_preserves_settings_symlink(self):
        for username in ("gytkk", "gyutak"):
            with self.subTest(username=username), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                home = root / username
                checkout = home / "ws"
                settings = checkout / "modules/claude/files/settings.json"
                settings.parent.mkdir(parents=True)
                settings.write_text('{"enabledPlugins":{"nixd-lsp@gytkk":true}}\n')
                original = settings.read_bytes()
                runtime = home / ".claude/settings.json"
                runtime.parent.mkdir()
                runtime.symlink_to(settings)
                binaries = root / "tools"
                bin_dir = binaries / "bin"
                bin_dir.mkdir(parents=True)
                for name in (
                    "timeout",
                    "readlink",
                    "cp",
                    "chmod",
                    "ln",
                    "mkdir",
                    "jq",
                    "rg",
                ):
                    (bin_dir / name).symlink_to(shutil.which(name))
                claude = bin_dir / "claude"
                claude.write_text("""#!/usr/bin/env bash
set -euo pipefail
if [ "$*" = 'plugin marketplace list' ]; then
  printf '%s\\n' anthropics/skills anthropics/claude-code anthropics/claude-plugins-official backnotprop/plannotator openai/codex-plugin-cc DietrichGebert/ponytail pbakaus/impeccable
elif [ "${1-} ${2-} ${3-}" = 'plugin marketplace add' ]; then
  source=$(jq -n --arg path "$4" '{source:"directory",path:$path}')
  jq --argjson source "$source" '.extraKnownMarketplaces.gytkk = {source:$source}' "$HOME/.claude/settings.json" > "$HOME/settings.tmp"
  cat "$HOME/settings.tmp" > "$HOME/.claude/settings.json"
  rm "$HOME/settings.tmp"
  mkdir -p "$HOME/.claude/plugins"
  jq -n --argjson source "$source" --arg path "$4" '{gytkk:{source:$source,installLocation:$path}}' > "$HOME/.claude/plugins/known_marketplaces.json"
elif [ "$*" != 'mcp list' ] && [ "$*" != 'mcp add -s user --transport http context7 https://mcp.context7.com/mcp' ]; then
  printf 'Unexpected fake Claude command: %s\\n' "$*" >&2
  exit 1
fi
""")
                claude.chmod(0o755)
                script = self.activation(checkout, binaries)
                result = subprocess.run(
                    ["bash", "-c", "set -euo pipefail\nrun() { :; }\n" + script],
                    env={**os.environ, "HOME": str(home)},
                    capture_output=True,
                    text=True,
                    check=False,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(settings.read_bytes(), original)
                self.assertTrue(runtime.is_symlink())
                self.assertEqual(runtime.resolve(), settings)
                registry = json.loads(
                    (home / ".claude/plugins/known_marketplaces.json").read_text()
                )
                self.assertEqual(
                    registry["gytkk"]["source"],
                    {
                        "source": "directory",
                        "path": str(checkout / "modules/claude/marketplace"),
                    },
                )


if __name__ == "__main__":
    unittest.main()
