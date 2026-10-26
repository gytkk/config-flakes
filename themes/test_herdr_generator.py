from __future__ import annotations

import copy
import re
import unittest
from pathlib import Path

import tomllib
from generate import contrast_ratio, herdr_theme_toml, hex_to_rgb, load_yaml, theme_context

ROOT = Path(__file__).resolve().parent
CUSTOM_COLOR_KEYS = {
    "accent",
    "panel_bg",
    "sidebar_bg",
    "active_row_bg",
    "selection_bg",
    "surface0",
    "surface1",
    "surface_dim",
    "overlay0",
    "overlay1",
    "text",
    "subtext0",
    "mauve",
    "green",
    "yellow",
    "red",
    "blue",
    "teal",
    "peach",
}


def ghostty_metal_contrast_ratio(foreground: str, background: str) -> float:
    # Ghostty 1.3.1 shaders.metal converts sRGB to Display P3 before contrast checks.
    # https://github.com/ghostty-org/ghostty/blob/v1.3.1/src/renderer/shaders/shaders.metal
    def luminance(color: str) -> float:
        rgb = [value / 255 for value in hex_to_rgb(color)]
        linear = [
            value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4
            for value in rgb
        ]
        for matrix in (
            (
                (0.4360747, 0.3850649, 0.1430804),
                (0.2225045, 0.7168786, 0.0606169),
                (0.0139322, 0.0971045, 0.7141733),
            ),
            (
                (2.40414768, -0.99010704, -0.39759019),
                (-0.84239098, 1.79905954, 0.01597023),
                (0.04838763, -0.09752546, 1.27393636),
            ),
        ):
            linear = [sum(a * b for a, b in zip(row, linear)) for row in matrix]
        return sum(a * b for a, b in zip(linear, (0.2126, 0.7152, 0.0722)))

    a, b = luminance(foreground), luminance(background)
    return (max(a, b) + 0.05) / (min(a, b) + 0.05)


class HerdrGeneratorTest(unittest.TestCase):
    def context(self, theme_id: str) -> dict:
        return theme_context(load_yaml(ROOT / "core" / f"{theme_id}.yaml"))

    def colors(self, ctx: dict) -> dict:
        return tomllib.loads(herdr_theme_toml(ctx, ROOT))["theme"]["custom"]

    def test_all_themes_render_supported_inline_colors_and_current_exports(
        self,
    ) -> None:
        for theme_path in sorted((ROOT / "core").glob("*.yaml")):
            with self.subTest(theme=theme_path.stem):
                ctx = theme_context(load_yaml(theme_path))
                rendered = herdr_theme_toml(ctx, ROOT)
                doc = tomllib.loads(rendered)
                self.assertEqual({"theme"}, set(doc))
                self.assertEqual({"name", "auto_switch", "custom"}, set(doc["theme"]))
                self.assertEqual("terminal", doc["theme"]["name"])
                self.assertIs(False, doc["theme"]["auto_switch"])
                colors = doc["theme"]["custom"]
                self.assertEqual(CUSTOM_COLOR_KEYS, set(colors))
                for key, color in colors.items():
                    self.assertIsInstance(color, str, key)
                    self.assertRegex(color, re.compile(r"^#[0-9a-fA-F]{6}$"), key)
                export = ROOT / "exports" / "herdr" / f"{ctx['meta']['id']}.toml"
                self.assertEqual(rendered, export.read_text())

    def test_muted_text_remains_readable_on_light_and_dark_row_backgrounds(
        self,
    ) -> None:
        for theme_path in sorted((ROOT / "core").glob("*.yaml")):
            theme_id = theme_path.stem
            with self.subTest(theme=theme_id):
                ctx = self.context(theme_id)
                colors = self.colors(ctx)
                for key in ("overlay0", "subtext0", "mauve"):
                    for background in (
                        ctx["roles"]["ui"]["bg"],
                        colors["panel_bg"],
                        colors["active_row_bg"],
                    ):
                        self.assertGreaterEqual(
                            contrast_ratio(colors[key], background), 4.5
                        )

    def test_surface_changes_propagate_from_canonical_roles(self) -> None:
        ctx = self.context("one-half-light")
        original = self.colors(ctx)
        self.assertEqual(ctx["roles"]["ui"]["panelBg"], original["panel_bg"])
        self.assertEqual(ctx["roles"]["ui"]["sidebarBg"], original["sidebar_bg"])

        changed = copy.deepcopy(ctx)
        changed["roles"]["ui"].update(panelBg="#eeeeee", bgAlt="#dddddd", fg="#202020")
        colors = self.colors(changed)
        self.assertEqual("#eeeeee", colors["panel_bg"])
        self.assertEqual("#dddddd", colors["surface1"])
        self.assertEqual("#202020", colors["text"])
        self.assertEqual("#202020", colors["overlay1"])
        self.assertNotEqual(original["panel_bg"], colors["panel_bg"])

    def test_current_space_and_agent_backgrounds_stand_out_with_readable_text(self) -> None:
        for theme_path in sorted((ROOT / "core").glob("*.yaml")):
            with self.subTest(theme=theme_path.stem):
                ctx = theme_context(load_yaml(theme_path))
                colors = self.colors(ctx)
                self.assertGreaterEqual(
                    contrast_ratio(colors["active_row_bg"], colors["sidebar_bg"]), 1.8
                )
                self.assertGreaterEqual(
                    contrast_ratio(colors["text"], colors["active_row_bg"]), 4.5
                )
                self.assertEqual(ctx["roles"]["ui"]["selection"], colors["selection_bg"])

    def test_selected_tabs_and_accent_text_have_readable_contrast(self) -> None:
        for theme_path in sorted((ROOT / "core").glob("*.yaml")):
            with self.subTest(theme=theme_path.stem):
                ctx = theme_context(load_yaml(theme_path))
                colors = self.colors(ctx)
                # Herdr renders selected tab labels with panel_bg on accent.
                self.assertGreaterEqual(
                    contrast_ratio(colors["panel_bg"], colors["accent"]), 4.5
                )
                original = ctx["roles"]["ui"]["borderActive"]
                if contrast_ratio(original, colors["panel_bg"]) >= 4.5:
                    self.assertEqual(original.lower(), colors["accent"].lower())

    def test_running_status_colors_survive_ghostty_contrast_on_focused_rows(self) -> None:
        repo = ROOT.parent
        ghostty = (repo / "modules/ghostty/files/config").read_text()
        minimum = float(
            re.search(r"^minimum-contrast\s*=\s*([\d.]+)$", ghostty, re.M)[1]
        )
        config = tomllib.loads((repo / "modules/herdr/files/config.toml").read_text())
        running_colors = {
            rule["fg"]
            for rows in config["ui"]["sidebar"]["agents"]["rows_by_agent"].values()
            for row in rows
            for token in row
            if isinstance(token, dict)
            for rule in token.get("rules", [])
            if rule.get("starts_with") == "●"
        }
        self.assertTrue(running_colors)
        self.assertEqual(
            {self.colors(self.context("one-half-light"))["green"]}, running_colors
        )
        for theme_path in sorted((ROOT / "core").glob("*.yaml")):
            colors = self.colors(self.context(theme_path.stem))
            for foreground in running_colors | {colors["green"]}:
                with self.subTest(theme=theme_path.stem, foreground=foreground):
                    for background in (colors["sidebar_bg"], colors["active_row_bg"]):
                        for contrast in (contrast_ratio, ghostty_metal_contrast_ratio):
                            with self.subTest(
                                background=background, renderer=contrast.__name__
                            ):
                                self.assertGreaterEqual(
                                    contrast(foreground, background), minimum
                                )

    def test_theme_running_green_has_margin_above_terminal_minimum_contrast(self) -> None:
        for theme_path in sorted((ROOT / "core").glob("*.yaml")):
            colors = self.colors(self.context(theme_path.stem))
            for key in ("sidebar_bg", "active_row_bg"):
                with self.subTest(theme=theme_path.stem, background=key):
                    self.assertGreaterEqual(
                        contrast_ratio(colors["green"], colors[key]), 1.8
                    )

    def test_metal_contrast_catches_the_previous_srgb_only_false_positive(self) -> None:
        self.assertGreater(contrast_ratio("#50a14f", "#b6b8bc"), 1.6)
        ratio = ghostty_metal_contrast_ratio("#50a14f", "#b6b8bc")
        self.assertAlmostEqual(1.5876304, ratio, places=6)
        self.assertLess(ratio, 1.6)


if __name__ == "__main__":
    unittest.main()
