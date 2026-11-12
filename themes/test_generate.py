from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parent


class GenerateTest(unittest.TestCase):
    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name).resolve() / "themes"
        shutil.copytree(ROOT, self.root, ignore=shutil.ignore_patterns("__pycache__"))

    def run_generator(self, *themes: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                str(self.root / "generate.py"),
                *(str(self.root / "core" / f"{theme}.yaml") for theme in themes),
            ],
            capture_output=True,
            text=True,
            check=False,
        )

    def export_snapshot(self) -> dict[Path, bytes]:
        return {
            path.relative_to(self.root / "exports"): path.read_bytes()
            for path in (self.root / "exports").rglob("*")
            if path.is_file()
        }

    def seed_old_exports(self) -> dict[Path, bytes]:
        for path in (self.root / "exports").rglob("*"):
            if path.is_file():
                path.write_text(f"old export: {path.name}\n")
        (self.root / "exports" / "unrelated.txt").write_text("preserve this file\n")
        return self.export_snapshot()

    def break_pi_template(self) -> None:
        path = self.root / "templates" / "pi" / "official-template.json"
        template = json.loads(path.read_text())
        template["document"]["colors"]["text"] = "$missing_test_slot"
        path.write_text(json.dumps(template))

    def test_late_app_failure_preserves_existing_exports(self) -> None:
        before = self.seed_old_exports()
        self.break_pi_template()

        result = self.run_generator("one-half-light")

        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("missing template slot: missing_test_slot", result.stderr)
        self.assertEqual(result.stdout, "")
        self.assertEqual(self.export_snapshot(), before)

    def test_late_app_failure_does_not_create_exports(self) -> None:
        shutil.rmtree(self.root / "exports")
        self.break_pi_template()

        result = self.run_generator("one-half-light")

        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("missing template slot: missing_test_slot", result.stderr)
        self.assertFalse((self.root / "exports").exists())

    def test_later_theme_validation_failure_preserves_all_exports(self) -> None:
        before = self.seed_old_exports()
        path = self.root / "core" / "ayu-mirage.yaml"
        path.write_text(path.read_text().replace("variant: dark", "variant: invalid"))

        result = self.run_generator("one-half-light", "ayu-mirage")

        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn(f"failed to generate {path}", result.stderr)
        self.assertIn("variant", result.stderr)
        self.assertEqual(result.stdout, "")
        self.assertEqual(self.export_snapshot(), before)

    def test_duplicate_theme_key_preserves_all_exports(self) -> None:
        before = self.seed_old_exports()
        path = self.root / "core" / "one-half-light.yaml"
        path.write_text(
            path.read_text().replace(
                "variant: light", "variant: dark\n  variant: light"
            )
        )

        result = self.run_generator("one-half-light")

        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("duplicate mapping key: variant", result.stderr)
        self.assertEqual(self.export_snapshot(), before)

    def test_later_theme_render_failure_preserves_all_exports(self) -> None:
        before = self.seed_old_exports()
        path = self.root / "overrides" / "zed" / "ayu-mirage.yaml"
        path.write_text("invalid override without mapping\n")

        result = self.run_generator("one-half-light", "ayu-mirage")

        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("failed to generate", result.stderr)
        self.assertIn("ayu-mirage.yaml", result.stderr)
        self.assertIn("expected mapping entry", result.stderr)
        self.assertEqual(result.stdout, "")
        self.assertEqual(self.export_snapshot(), before)

    def test_default_generation_preserves_export_bytes_and_prints_paths(self) -> None:
        expected = self.export_snapshot()
        shutil.rmtree(self.root / "exports")

        result = self.run_generator()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, "")
        self.assertEqual(self.export_snapshot(), expected)
        self.assertCountEqual(
            result.stdout.splitlines(),
            [str(Path("themes/exports") / path) for path in expected],
        )

    def test_nvim_and_zellij_reject_invalid_overrides_before_writing(self) -> None:
        for app, body, invalid in (
            (
                "nvim",
                'groups:\n  Normal:\n    fg: "{roles.ui.fg}"\nlinks:\n',
                (
                    (
                        'fg: "{roles.ui.fg}"',
                        'made_up_attribute: "true"',
                        "unsupported attr",
                    ),
                    ('fg: "{roles.ui.fg}"', "fg: 123", "supported override scalar"),
                ),
            ),
            (
                "zellij",
                'components:\n  text_selected:\n    base: "{roles.ui.fg}"\nplayers:\n',
                (
                    (
                        "text_selected:",
                        "made_up_component:",
                        "supported zellij component",
                    ),
                    ("base:", "made_up_attribute:", "unsupported attr"),
                    (
                        '"{roles.ui.fg}"',
                        '"invalid-color"',
                        "hex, palette ref, or role ref",
                    ),
                    (
                        "players:",
                        'players:\n  player_11: "#123456"',
                        "invalid player key",
                    ),
                ),
            ),
        ):
            source = f"version: 1\nmeta:\n  app: {app}\n  theme: ayu-mirage\n  variant: dark\n{body}"
            path = self.root / "overrides" / app / "ayu-mirage.yaml"
            for old, new, error in (
                *invalid,
                (f"app: {app}", "app: ghostty", "meta.app"),
                (
                    "theme: ayu-mirage",
                    "theme: another-theme",
                    "metadata does not match",
                ),
                ("variant: dark", "variant: light", "metadata does not match"),
            ):
                with self.subTest(app=app, invalid=new):
                    before = self.seed_old_exports()
                    path.write_text(source.replace(old, new))
                    result = self.run_generator("one-half-light", "ayu-mirage")
                    self.assertEqual(result.returncode, 1, result.stderr)
                    self.assertIn(str(path), result.stderr)
                    self.assertIn(error, result.stderr)
                    self.assertEqual(result.stdout, "")
                    self.assertEqual(self.export_snapshot(), before)
            path.unlink()


if __name__ == "__main__":
    unittest.main()
