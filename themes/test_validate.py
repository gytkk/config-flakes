from __future__ import annotations

import runpy
import unittest
from pathlib import Path


VALIDATOR = runpy.run_path(str(Path(__file__).with_name("validate.py")))
parse_yaml_subset = VALIDATOR["parse_yaml_subset"]
ParseError = VALIDATOR["ParseError"]


class YamlMappingTest(unittest.TestCase):
    def test_duplicate_keys_are_rejected_for_each_value_kind(self) -> None:
        for document in (
            "version: 1\nversion: 2\n",
            "meta:\n  variant: dark\n  variant: light\n",
            "meta:\n  variant: dark\nmeta:\n  variant: light\n",
            "description: first\ndescription: >-\n  second\n",
        ):
            with self.subTest(document=document):
                with self.assertRaises(ParseError):
                    parse_yaml_subset(document)

    def test_same_key_in_separate_mappings_is_allowed(self) -> None:
        self.assertEqual(
            parse_yaml_subset("first:\n  color: red\nsecond:\n  color: blue\n"),
            {"first": {"color": "red"}, "second": {"color": "blue"}},
        )


if __name__ == "__main__":
    unittest.main()
