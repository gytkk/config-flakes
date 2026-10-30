#!/usr/bin/env python3
from __future__ import annotations

import unittest
from pathlib import Path

from generate import load_yaml, pi_theme_doc, theme_context


ROOT = Path(__file__).resolve().parent


class PiGeneratorTest(unittest.TestCase):
    def test_one_half_light_maps_canonical_ui_and_syntax_colors(self) -> None:
        theme = load_yaml(ROOT / "core" / "one-half-light.yaml")
        doc = pi_theme_doc(theme_context(theme), ROOT)

        self.assertEqual("one-half-light", doc["name"])
        self.assertEqual("#383a42", doc["colors"]["text"])
        self.assertEqual("#bfceff", doc["colors"]["selectedBg"])
        self.assertEqual("#0184bc", doc["colors"]["syntaxFunction"])
        self.assertEqual("#50a14f", doc["colors"]["syntaxString"])
        self.assertEqual("#e45649", doc["colors"]["toolDiffRemoved"])
        self.assertEqual("#fafbfc", doc["export"]["pageBg"])


if __name__ == "__main__":
    unittest.main()
