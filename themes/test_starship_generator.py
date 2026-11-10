from __future__ import annotations

import tempfile
import tomllib
import unittest
from pathlib import Path

from generate import contrast_ratio, load_yaml, starship_theme_toml, theme_context
from validate import Validator
from validate_overrides import validate_starship_override


ROOT = Path(__file__).resolve().parent


class StarshipGeneratorTest(unittest.TestCase):
    def test_one_half_light_text_contrast_and_canonical_status_colors(self) -> None:
        ctx = theme_context(load_yaml(ROOT / "core" / "one-half-light.yaml"))
        doc = tomllib.loads(starship_theme_toml(ctx, ROOT))
        palette = doc["palettes"][doc["palette"]]
        for layer in ("layer1", "layer2", "layer3"):
            self.assertGreaterEqual(contrast_ratio(palette["text"], palette[layer]), 4.5)
        for slot, color in (("pine", "green"), ("gold", "yellow"), ("love", "red")):
            with self.subTest(slot=slot):
                self.assertEqual(palette[slot], ctx["palette"][color])

    def test_other_themes_keep_their_committed_exports(self) -> None:
        for path in (ROOT / "core").glob("*.yaml"):
            if path.stem == "one-half-light":
                continue
            with self.subTest(theme=path.stem):
                rendered = starship_theme_toml(theme_context(load_yaml(path)), ROOT)
                self.assertEqual((ROOT / "exports" / "starship" / f"{path.stem}.toml").read_text(), rendered)

    def test_override_rejects_unknown_keys_and_nonstring_settings(self) -> None:
        source = (ROOT / "overrides" / "starship" / "TEMPLATE.yaml").read_text()
        invalid = [
            source.replace("foam:", "unknown_color:"),
            source.replace("{palette.blue}", "not-a-color"),
            source.replace("directory:", "unknown_module:"),
            source.replace("style:", "unknown_setting:"),
            source.replace('style: "fg:text bg:layer1"', "truncation_length: 3"),
        ]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "override.yaml"
            for candidate in invalid:
                with self.subTest(candidate=candidate):
                    path.write_text(candidate)
                    validator = Validator()
                    validate_starship_override(validator, path)
                    self.assertTrue(validator.errors)


if __name__ == "__main__":
    unittest.main()
