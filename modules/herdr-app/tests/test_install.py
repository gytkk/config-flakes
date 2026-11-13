import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("herdr_app_install", ROOT / "install.py")
assert SPEC is not None and SPEC.loader is not None
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
            original_settings = {"zoom": 0.9, "minimum_contrast": 1.0, "sounds": {"enabled": True}}
            settings.write_text(json.dumps(original_settings))
            layouts = home / "layouts"
            layouts.mkdir()
            layout = {"id": "root", "custom": "keep", "terminal_padding": [14, 12], "children": [
                {"id": "main", "content": {"tab": "active", "pane": {
                    "gap": 8, "radius": 10, "header": {"height": 30},
                    "border": {"width": 4, "color": "border"},
                    "focus": {"width": 4, "color": "#123456"}
                }}},
                {"id": "status", "content": {"mod": "workspace-status"}, "size": {"px": 28}},
                {"id": "work-body", "type": "row", "padding": 8, "gap": 8},
                {"id": "spaces", "content": {"mod": "spaces"}},
                {"id": "personal", "content": {"mod": "custom-spaces"}}
            ]}
            (layouts / "main.json").write_text(json.dumps(layout))
            installer.install(ROOT / "files", home)
            merged = json.loads(keys.read_text())
            self.assertEqual(merged["global"]["cmd-e"], "app.palette")
            for key in ("alt-up", "alt-["):
                self.assertEqual(merged["global"][key], "space.prev")
            for key in ("alt-down", "alt-]"):
                self.assertEqual(merged["global"][key], "space.next")
            self.assertEqual(merged["terminal"], original["terminal"])
            self.assertEqual(json.loads(settings.read_text()), original_settings)
            self.assertFalse((home / "settings.json.home-manager.bak").exists())
            personal = json.loads((layouts / "tui.json").read_text())
            self.assertEqual(personal["custom"], "keep")
            self.assertEqual(personal["terminal_padding"], [4, 4])
            self.assertEqual(personal["children"][0]["content"]["pane"],
                             {"gap": 4, "radius": 10, "header": {"height": 30},
                              "border": {"width": 0.5, "color": "border"},
                              "focus": {"width": 0.5, "color": "#123456"}})
            self.assertEqual(personal["children"][1]["content"]["mod"], "tui-prefix")
            self.assertEqual(personal["children"][2],
                             {"id": "work-body", "type": "row", "padding": 4, "gap": 8})
            self.assertEqual(json.loads((layouts / "main.json").read_text()), layout)
            self.assertEqual(personal["children"][3]["content"]["mod"], "tui-spaces")
            self.assertEqual(personal["children"][4]["content"]["mod"], "custom-spaces")
            personal["custom"] = "personal change"
            personal["children"][0]["content"]["pane"].update(gap=12, radius=6)
            personal["terminal_padding"] = [8, 6]
            personal["children"][0]["content"]["pane"]["border"]["width"] = 2
            personal["children"][0]["content"]["pane"]["focus"]["width"] = 3
            personal["children"][2]["padding"] = 12
            (layouts / "tui.json").write_text(json.dumps(personal))
            backup = home / "keys.json.home-manager.bak"
            self.assertEqual(json.loads(backup.read_text()), original)
            changed = keys.stat().st_mtime_ns
            settings_changed = settings.stat().st_mtime_ns
            installer.install(ROOT / "files", home)
            self.assertEqual(keys.stat().st_mtime_ns, changed)
            self.assertEqual(settings.stat().st_mtime_ns, settings_changed)
            self.assertEqual(json.loads(backup.read_text()), original)
            reapplied = json.loads((layouts / "tui.json").read_text())
            self.assertEqual(reapplied["custom"], "personal change")
            self.assertEqual(reapplied["terminal_padding"], [4, 4])
            self.assertEqual(reapplied["children"][0]["content"]["pane"],
                             {"gap": 4, "radius": 6, "header": {"height": 30},
                              "border": {"width": 0.5, "color": "border"},
                              "focus": {"width": 0.5, "color": "#123456"}})
            self.assertEqual(reapplied["children"][2],
                             {"id": "work-body", "type": "row", "padding": 4, "gap": 8})

    def test_preserves_settings_symlink_and_target(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            (home / "keys.json").write_text("{}")
            (home / "layouts").mkdir()
            (home / "layouts/main.json").write_text(json.dumps({
                "id": "status", "content": {"mod": "workspace-status"}
            }))
            target = home / "personal-settings.json"
            original = '{"colors": {"cursor": "#0891B2"}, "font_size": 15}\n'
            target.write_text(original)
            settings = home / "settings.json"
            settings.symlink_to(target)
            installer.install(ROOT / "files", home)
            self.assertTrue(settings.is_symlink())
            self.assertEqual(settings.readlink(), target)
            self.assertEqual(target.read_text(), original)
            self.assertFalse((home / "settings.json.home-manager.bak").exists())

    def test_windows_settings_copy_and_backup_are_idempotent(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            (home / "keys.json").write_text("{}")
            (home / "layouts").mkdir()
            (home / "layouts/main.json").write_text(json.dumps({
                "id": "status", "content": {"mod": "workspace-status"}
            }))
            settings = home / "settings.json"
            original = '{"zoom": 1.2}\n'
            settings.write_text(original)
            expected = (ROOT / "files/settings.json").read_bytes()
            installer.install(ROOT / "files", home, copy_settings=True)
            self.assertEqual(settings.read_bytes(), expected)
            self.assertFalse(settings.is_symlink())
            self.assertEqual(json.loads(settings.read_text())["colors"]["cursor"], "#0891B2")
            backup = home / "settings.json.home-manager.bak"
            self.assertEqual(backup.read_text(), original)
            modified = settings.stat().st_mtime_ns
            installer.install(ROOT / "files", home, copy_settings=True)
            self.assertEqual(settings.stat().st_mtime_ns, modified)
            self.assertEqual(backup.read_text(), original)
            settings.unlink()
            installer.install(ROOT / "files", home, copy_settings=True)
            self.assertEqual(settings.read_bytes(), expected)

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
