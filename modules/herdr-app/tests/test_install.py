import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("herdr_app_install", ROOT / "install.py")
installer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(installer)


class InstallTests(unittest.TestCase):
    def test_preserves_personal_keys_and_settings_and_keeps_original_backup(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            original = {"global": {"cmd-[": "agent.prev", "cmd-e": "app.palette"}, "terminal": {"ctrl-r": "terminal"}}
            keys = home / "keys.json"
            keys.write_text(json.dumps(original))
            settings = home / "settings.json"
            settings.write_text('{"zoom": 0.9}')
            layouts = home / "layouts"
            layouts.mkdir()
            layout = {"id": "root", "custom": "keep", "children": [
                {"id": "status", "content": {"mod": "workspace-status"}, "size": {"px": 28}}
            ]}
            (layouts / "main.json").write_text(json.dumps(layout))
            installer.install(ROOT / "files", home)
            merged = json.loads(keys.read_text())
            self.assertEqual(merged["global"]["cmd-e"], "app.palette")
            self.assertEqual(merged["terminal"], original["terminal"])
            self.assertEqual(settings.read_text(), '{"zoom": 0.9}')
            personal = json.loads((layouts / "tui.json").read_text())
            self.assertEqual(personal["custom"], "keep")
            self.assertEqual(json.loads((layouts / "main.json").read_text()), layout)
            personal["custom"] = "personal change"
            (layouts / "tui.json").write_text(json.dumps(personal))
            backup = home / "keys.json.home-manager.bak"
            self.assertEqual(json.loads(backup.read_text()), original)
            changed = keys.stat().st_mtime_ns
            installer.install(ROOT / "files", home)
            self.assertEqual(keys.stat().st_mtime_ns, changed)
            self.assertEqual(json.loads(backup.read_text()), original)
            self.assertEqual(json.loads((layouts / "tui.json").read_text())["custom"], "personal change")

    def test_invalid_keys_leave_existing_mod_and_configuration_untouched(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            keys = home / "keys.json"
            keys.write_text('{"terminal": []}')
            mod = home / "mods/tui-prefix/main.js"
            mod.parent.mkdir(parents=True)
            mod.write_text("personal copy")
            with self.assertRaises(ValueError):
                installer.install(ROOT / "files", home)
            self.assertEqual(keys.read_text(), '{"terminal": []}')
            self.assertEqual(mod.read_text(), "personal copy")
            self.assertFalse((home / "keys.json.home-manager.bak").exists())


if __name__ == "__main__":
    unittest.main()
