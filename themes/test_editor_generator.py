from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from generate import load_yaml, nvim_lua, theme_context, zed_theme_doc
from validate import Validator
from validate_overrides import validate_zed_override


ROOT = Path(__file__).resolve().parent


class EditorGeneratorTest(unittest.TestCase):
    def test_zed_patch_preserves_alpha_and_inherits_unpatched_attributes(self) -> None:
        ctx = theme_context(load_yaml(ROOT / "core" / "one-half-light.yaml"))
        original = zed_theme_doc(ctx, ROOT)["themes"][0]["style"]
        source = (ROOT / "overrides" / "zed" / "TEMPLATE.yaml").read_text()
        source = source.replace("example-theme", "one-half-light").replace("variant: dark", "variant: light")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "templates").symlink_to(ROOT / "templates", target_is_directory=True)
            overrides = root / "overrides" / "zed"
            overrides.mkdir(parents=True)
            (overrides / "one-half-light.yaml").write_text(source)
            style = zed_theme_doc(ctx, root)["themes"][0]["style"]
        self.assertEqual(style["players"][0]["selection"], "#47474780")
        self.assertEqual(style["players"][0]["cursor"], ctx["roles"]["ui"]["cursor"])
        self.assertEqual(style["players"][0]["background"], original["players"][0]["background"])
        self.assertEqual(style["players"][1:], original["players"][1:])
        self.assertEqual(style["syntax"]["keyword"]["color"], ctx["palette"]["cyan"])
        self.assertEqual(style["syntax"]["keyword"]["font_weight"], original["syntax"]["keyword"]["font_weight"])
        self.assertEqual(style["editor.background"], original["editor.background"])

    def test_zed_override_rejects_invalid_surface_values_and_metadata(self) -> None:
        source = (ROOT / "overrides" / "zed" / "TEMPLATE.yaml").read_text()
        invalid = [
            source.replace("app: zed", "app: nvim"),
            source.replace("text.accent:", "unknown.style:"),
            source.replace("#47474780", "#4747478"),
            source.replace("cursor:", "unknown_player_attr:"),
            source.replace("player_1:", "player_0:"),
            source.replace("player_1:", "player_99:"),
            source.replace('color: "{palette.cyan}"', "font_style: invalid"),
            source.replace('color: "{palette.cyan}"', "font_weight: 450"),
            source.replace('color: "{palette.cyan}"', "unknown_attribute: null"),
        ]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "override.yaml"
            for candidate in invalid:
                with self.subTest(candidate=candidate):
                    path.write_text(candidate)
                    validator = Validator()
                    validate_zed_override(validator, path)
                    self.assertTrue(validator.errors)

            root = Path(directory)
            (root / "templates").symlink_to(ROOT / "templates", target_is_directory=True)
            overrides = root / "overrides" / "zed"
            overrides.mkdir(parents=True)
            (overrides / "vira-graphene.yaml").write_text(source)
            ctx = theme_context(load_yaml(ROOT / "core" / "vira-graphene.yaml"))
            with self.assertRaisesRegex(RuntimeError, "metadata does not match"):
                zed_theme_doc(ctx, root)

    def test_vira_graphene_zed_keeps_original_editor_colors(self) -> None:
        ctx = theme_context(load_yaml(ROOT / "core" / "vira-graphene.yaml"))
        style = zed_theme_doc(ctx, ROOT)["themes"][0]["style"]
        self.assertEqual(style["players"][0]["selection"], "#47474780")
        self.assertEqual(style["players"][0]["cursor"].lower(), "#ffcc00")
        self.assertEqual(style["syntax"]["keyword"]["color"].lower(), "#89ddff")
        self.assertEqual(style["syntax"]["boolean"]["color"], "#ff9cac")
        self.assertEqual(style["syntax"]["punctuation.bracket"]["color"].lower(), "#89ddff")
        self.assertIsNone(style["syntax"]["title"]["font_weight"])

    def test_other_themes_keep_their_committed_editor_exports(self) -> None:
        for path in (ROOT / "core").glob("*.yaml"):
            if path.stem == "vira-graphene":
                continue
            with self.subTest(theme=path.stem):
                ctx = theme_context(load_yaml(path))
                self.assertEqual(nvim_lua(ctx, ROOT), (ROOT / "exports" / "nvim" / f"{path.stem}.lua").read_text())
                expected = json.loads((ROOT / "exports" / "zed" / f"{path.stem}.json").read_text())
                self.assertEqual(zed_theme_doc(ctx, ROOT), expected)


if __name__ == "__main__":
    unittest.main()
