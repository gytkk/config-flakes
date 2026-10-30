from __future__ import annotations

import shutil
import tempfile
import unittest
from pathlib import Path

from generate import contrast_ratio, ghostty_theme_conf, load_yaml, mix, theme_context
from validate_overrides import valid_ghostty_slot_value

ROOT = Path(__file__).resolve().parent


class GhosttyGeneratorTest(unittest.TestCase):
    def test_light_secondary_text_remains_readable_when_faint(self) -> None:
        ctx = theme_context(load_yaml(ROOT / "core" / "one-half-light.yaml"))
        conf = ghostty_theme_conf(ctx, ROOT)
        settings = dict(
            line.split(" = ", 1) for line in conf.splitlines() if " = " in line
        )
        opacity = float(settings.get("faint-opacity", "0.5"))
        background = settings["background"]
        foreground = ctx["roles"]["ansi"]["brightBlack"]
        self.assertGreaterEqual(
            contrast_ratio(mix(foreground, background, 1 - opacity), background), 4.5
        )
        self.assertIn("palette = 7=#bababa", conf)
        self.assertIn("palette = 15=#ffffff", conf)

    def test_dark_themes_keep_default_faint_opacity(self) -> None:
        for path in (ROOT / "core").glob("*.yaml"):
            ctx = theme_context(load_yaml(path))
            if ctx["meta"]["variant"] == "dark":
                with self.subTest(theme=path.stem):
                    self.assertIn(
                        "faint-opacity = 0.5\n", ghostty_theme_conf(ctx, ROOT)
                    )

    def test_faint_opacity_override_has_a_bounded_numeric_contract(self) -> None:
        for value in ("0", "0.5", "0.85", "1"):
            self.assertTrue(valid_ghostty_slot_value("faint_opacity", value), value)
        for value in (
            "-0.1",
            "1.1",
            "nan",
            "inf",
            "#ffffff",
            "{palette.bg}",
            "0.5\nforeground = #ffffff",
        ):
            self.assertFalse(valid_ghostty_slot_value("faint_opacity", value), value)

    def test_generator_rejects_invalid_opacity_before_rendering(self) -> None:
        ctx = theme_context(load_yaml(ROOT / "core" / "one-half-light.yaml"))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            shutil.copytree(
                ROOT / "templates" / "ghostty", root / "templates" / "ghostty"
            )
            overrides = root / "overrides" / "ghostty"
            overrides.mkdir(parents=True)
            source = (
                ROOT / "overrides" / "ghostty" / "one-half-light.yaml"
            ).read_text()
            (overrides / "one-half-light.yaml").write_text(
                source.replace('"0.85"', '"1.5"')
            )
            with self.assertRaisesRegex(RuntimeError, "opacity from 0 to 1"):
                ghostty_theme_conf(ctx, root)

    def test_palette_override_template_resolves_color_references(self) -> None:
        ctx = theme_context(load_yaml(ROOT / "core" / "one-half-light.yaml"))
        source = (ROOT / "overrides" / "ghostty" / "TEMPLATE.yaml").read_text()
        source = source.replace("theme-id", "one-half-light").replace(
            "variant: dark", "variant: light"
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "templates").symlink_to(
                ROOT / "templates", target_is_directory=True
            )
            overrides = root / "overrides" / "ghostty"
            overrides.mkdir(parents=True)
            path = overrides / "one-half-light.yaml"
            for entry, expected in (
                ("0={roles.ansi.black}", f"0={ctx['roles']['ansi']['black']}"),
                ("0={palette.blue}", f"0={ctx['palette']['blue']}"),
                ("0=#123456", "0=#123456"),
                (
                    "15={roles.ansi.brightWhite}",
                    f"15={ctx['roles']['ansi']['brightWhite']}",
                ),
            ):
                with self.subTest(entry=entry):
                    path.write_text(
                        source.replace(
                            "palette_0_entry", f"palette_{entry.split('=')[0]}_entry"
                        ).replace("0={roles.ansi.black}", entry)
                    )
                    conf = ghostty_theme_conf(ctx, root)
                    self.assertIn(f"palette = {expected}\n", conf)
                    self.assertIn(
                        f"cursor-color = {ctx['roles']['ui']['cursor']}\n", conf
                    )
                    self.assertNotIn("{roles.", conf)
                    self.assertNotIn("{palette.", conf)

    def test_palette_override_rejects_malformed_entries_and_missing_references(
        self,
    ) -> None:
        ctx = theme_context(load_yaml(ROOT / "core" / "one-half-light.yaml"))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "templates").symlink_to(
                ROOT / "templates", target_is_directory=True
            )
            overrides = root / "overrides" / "ghostty"
            overrides.mkdir(parents=True)
            path = overrides / "one-half-light.yaml"
            for entry, error in (
                ("0=not-a-color", RuntimeError),
                ("16=#123456", RuntimeError),
                ("0={roles.ansi.missing}", KeyError),
                ("0={palette.missing}", KeyError),
            ):
                with self.subTest(entry=entry):
                    path.write_text(
                        f'version: 1\nmeta:\n  app: ghostty\n  theme: one-half-light\n  variant: light\nslots:\n  palette_0_entry: "{entry}"\n'
                    )
                    with self.assertRaises(error):
                        ghostty_theme_conf(ctx, root)


if __name__ == "__main__":
    unittest.main()
