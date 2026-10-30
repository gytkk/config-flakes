"""Run evaluated activation snippets against temporary homes, without a switch."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


class ConfigActivationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.home = self.root / "home"
        self.home.mkdir()
        self.source = self.root / "source"
        self.source.mkdir()
        self.settings = self.source / "config/modules/zed/files/settings.json"
        self.settings.parent.mkdir(parents=True)
        self.settings.write_text('{"theme": "new",}\n')  # Zed accepts JSONC.
        self.keymap = self.source / "keymap.json"
        self.keymap.write_text("[]\n")
        self.themes = self.source / "themes"
        self.themes.mkdir()
        (self.themes / "test.json").write_text('{"name":"new"}\n')
        self.extensions = self.source / "extensions"
        (self.extensions / "test").mkdir(parents=True)
        (self.extensions / "test/new").write_text("new")
        self.zed = self.home / "AppData/Roaming/Zed"
        self.installed = self.home / "AppData/Local/Zed/extensions/installed"
        self.terminal = (
            self.home
            / "AppData/Local/Packages/Microsoft.WindowsTerminal_8wekyb3d8bbwe/LocalState/settings.json"
        )
        self.managed = self.home / ".config/aerospace/aerospace.toml"
        self.legacy = self.home / ".aerospace.toml"

    def activation(self, module, name):
        expression = """
          let
            lib = {
              mkIf = condition: value: if condition then value else {};
              mkMerge = builtins.foldl' (a: b: a // b) {};
              hm.dag.entryAfter = after: data: { inherit after data; };
              escapeShellArg = value: "'" + value + "'";
            };
            result = import MODULE {
              inherit lib;
              config = {
                modules = { aerospace.enable = true; zed.enable = true; windowsTerminal.enable = true; };
                home.homeDirectory = HOME_PATH;
                xdg.configHome = HOME_PATH + "/.config";
              };
              pkgs = {
                stdenv.hostPlatform.isDarwin = true;
                writeText = name: text: KEYMAP;
                runCommand = name: attrs: script: EXTENSIONS;
                jq = JQ;
                diffutils = DIFFUTILS;
              };
              flakeDirectory = SOURCE;
              themeExports.mutableDir = app: THEMES;
              username = "activation-test";
              isWSL = true;
            };
          in result.config.home.activation.NAME
        """
        for key, value in {
            "MODULE": str(ROOT / "config/modules" / module / "default.nix"),
            "HOME_PATH": json.dumps(str(self.home)),
            "KEYMAP": json.dumps(str(self.keymap)),
            "EXTENSIONS": json.dumps(str(self.extensions)),
            "JQ": json.dumps(str(Path(shutil.which("jq")).parent.parent)),
            "DIFFUTILS": json.dumps(str(Path(shutil.which("cmp")).parent.parent)),
            "SOURCE": json.dumps(str(self.source)),
            "THEMES": json.dumps(str(self.themes)),
            "NAME": name,
        }.items():
            expression = expression.replace(key, value)
        result = subprocess.run(
            ["nix", "eval", "--impure", "--json", "--expr", expression],
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        activation = json.loads(result.stdout)
        activation["data"] = activation["data"].replace(
            "/mnt/c/Users/activation-test", str(self.home)
        )
        return activation

    def run_activation(self, module, name, dry=False, setup=""):
        script = self.activation(module, name)["data"]
        environment = {
            key: value for key, value in os.environ.items() if key != "DRY_RUN"
        }
        if dry:
            environment["DRY_RUN"] = "1"
        return subprocess.run(
            [
                "bash",
                "-c",
                """set -euo pipefail
run() { if [ "${DRY_RUN+x}" = x ]; then printf 'would run: %s\\n' "$*"; else "$@"; fi; }
"""
                + setup
                + "\n"
                + script,
            ],
            env=environment,
            capture_output=True,
            text=True,
        )

    def snapshot(self):
        return {
            str(path.relative_to(self.home)): (
                os.readlink(path)
                if path.is_symlink()
                else path.read_bytes()
                if path.is_file()
                else None
            )
            for path in self.home.rglob("*")
        }

    def populate_zed(self):
        self.zed.mkdir(parents=True)
        (self.zed / "settings.json").write_text("original settings")
        (self.zed / "keymap.json").write_text("original keymap")
        (self.installed / "test").mkdir(parents=True)
        (self.installed / "test/old").write_text("old extension")

    def test_aerospace_runs_after_link_generation(self):
        self.assertIn(
            "linkGeneration",
            self.activation("aerospace", "aerospaceLegacyConfig")["after"],
        )

    def test_aerospace_first_dry_run_without_managed_link(self):
        before = self.snapshot()
        result = self.run_activation("aerospace", "aerospaceLegacyConfig", dry=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.snapshot(), before)

    def test_aerospace_dry_run_preserves_legacy(self):
        self.managed.parent.mkdir(parents=True)
        self.managed.write_text("managed")
        for content in ("managed", "custom"):
            self.legacy.write_text(content)
            before = self.snapshot()
            result = self.run_activation("aerospace", "aerospaceLegacyConfig", dry=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(self.snapshot(), before)

    def test_aerospace_migration_and_reapplication(self):
        self.managed.parent.mkdir(parents=True)
        self.managed.write_text("managed")
        result = self.run_activation("aerospace", "aerospaceLegacyConfig")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.legacy.write_text("custom")
        for _ in range(2):
            result = self.run_activation("aerospace", "aerospaceLegacyConfig")
            self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(self.legacy.exists())
        self.assertEqual(
            self.legacy.with_suffix(".toml.pre-xdg-backup").read_text(), "custom"
        )

    def test_aerospace_conflicting_backup_preserves_both_files(self):
        self.managed.parent.mkdir(parents=True)
        self.managed.write_text("managed")
        self.legacy.write_text("custom")
        self.legacy.with_suffix(".toml.pre-xdg-backup").write_text("older custom")
        before = self.snapshot()
        result = self.run_activation("aerospace", "aerospaceLegacyConfig")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.snapshot(), before)

    def test_zed_dry_run_preserves_home(self):
        for populated in (False, True):
            if populated:
                self.populate_zed()
            before = self.snapshot()
            result = self.run_activation("zed", "zedWindowsConfig", dry=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(self.snapshot(), before)

    def test_zed_missing_source_preserves_settings(self):
        self.populate_zed()
        self.settings.unlink()
        result = self.run_activation("zed", "zedWindowsConfig")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.zed / "settings.json").read_text(), "original settings")

    def test_zed_partial_copy_preserves_settings(self):
        self.populate_zed()
        result = self.run_activation(
            "zed",
            "zedWindowsConfig",
            setup='''
cp() {
  if [ "$1" = "'''
            + str(self.settings)
            + """" ]; then
    printf partial > "${@: -1}"
    return 42
  fi
  command cp "$@"
}
""",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.zed / "settings.json").read_text(), "original settings")

    def test_zed_missing_keymap_preserves_settings(self):
        self.populate_zed()
        self.keymap.unlink()
        result = self.run_activation("zed", "zedWindowsConfig")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.zed / "settings.json").read_text(), "original settings")
        self.assertEqual((self.zed / "keymap.json").read_text(), "original keymap")

    def test_zed_partial_extension_copy_preserves_old_extension(self):
        self.populate_zed()
        result = self.run_activation(
            "zed",
            "zedWindowsConfig",
            setup="""
cp() {
  if [ "$1" = -rL ]; then
    mkdir "${@: -1}"
    printf partial > "${@: -1}/partial"
    return 42
  fi
  command cp "$@"
}
""",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.installed / "test/old").read_text(), "old extension")
        self.assertFalse(list(self.installed.glob(".home-manager.*")))

    def test_zed_failed_extension_rename_restores_previous_directory(self):
        self.populate_zed()
        result = self.run_activation(
            "zed",
            "zedWindowsConfig",
            setup="""
mv() {
  case "$2" in */new) return 42 ;; esac
  command mv "$@"
}
""",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.installed / "test/old").read_text(), "old extension")
        self.assertFalse(list(self.installed.glob(".home-manager.*")))

    def test_zed_failed_extension_rollback_retains_backup(self):
        self.populate_zed()
        result = self.run_activation(
            "zed",
            "zedWindowsConfig",
            setup="""
mv() {
  case "$2" in */new|*/previous) return 42 ;; esac
  command mv "$@"
}
""",
        )
        self.assertNotEqual(result.returncode, 0)
        backup = list(self.installed.glob(".home-manager.*/previous/old"))
        self.assertEqual(len(backup), 1)
        self.assertEqual(backup[0].read_text(), "old extension")
        self.assertIn(str(backup[0].parent), result.stderr)

    def test_zed_replaces_files_and_refreshes_backup(self):
        self.populate_zed()
        result = self.run_activation("zed", "zedWindowsConfig")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            (self.zed / "settings.json").read_bytes(), self.settings.read_bytes()
        )
        self.assertEqual(
            (self.zed / "settings.json.bak").read_text(), "original settings"
        )
        self.assertFalse((self.installed / "test/old").exists())
        (self.zed / "settings.json").write_text("recent edit")
        result = self.run_activation("zed", "zedWindowsConfig")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.zed / "settings.json.bak").read_text(), "recent edit")

    def test_terminal_dry_run_never_creates_temporary_file(self):
        self.terminal.parent.mkdir(parents=True)
        for content in ('{"profiles":[]}', "invalid json"):
            self.terminal.write_text(content)
            before = self.snapshot()
            result = self.run_activation(
                "windows-terminal", "windowsTerminalHerdrKeybindings", dry=True
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(self.snapshot(), before)

    def test_terminal_invalid_json_preserves_settings(self):
        self.terminal.parent.mkdir(parents=True)
        self.terminal.write_text("invalid json")
        before = self.snapshot()
        result = self.run_activation(
            "windows-terminal", "windowsTerminalHerdrKeybindings"
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.snapshot(), before)

    def test_terminal_failed_rename_preserves_settings(self):
        self.terminal.parent.mkdir(parents=True)
        self.terminal.write_text('{"profiles":[]}')
        before = self.terminal.read_bytes()
        result = self.run_activation(
            "windows-terminal",
            "windowsTerminalHerdrKeybindings",
            setup="""
mv() {
  case "$2" in */settings.json) return 42 ;; esac
  command mv "$@"
}
""",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.terminal.read_bytes(), before)
        self.assertEqual(
            Path(str(self.terminal) + ".home-manager.bak").read_bytes(), before
        )
        self.assertEqual(len(list(self.terminal.parent.iterdir())), 2)

    def test_terminal_preserves_profiles_and_latest_backup(self):
        self.terminal.parent.mkdir(parents=True)
        self.terminal.write_text('{"profiles":{"list":[{"name":"custom"}]}}')
        for index in range(2):
            before = self.terminal.read_bytes()
            result = self.run_activation(
                "windows-terminal", "windowsTerminalHerdrKeybindings"
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                Path(str(self.terminal) + ".home-manager.bak").read_bytes(), before
            )
            updated = json.loads(self.terminal.read_text())
            self.assertEqual(updated["profiles"]["list"][0]["name"], "custom")
            updated["customSetting"] = index
            self.terminal.write_text(json.dumps(updated))


if __name__ == "__main__":
    unittest.main()
