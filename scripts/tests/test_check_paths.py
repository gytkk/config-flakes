"""Distinguish flake input names from repository paths in documentation."""

import runpy
import tempfile
import unittest
from pathlib import Path

CHECK_DOCUMENT = runpy.run_path(
    str(Path(__file__).resolve().parents[1] / "check-paths.py")
)["check_document"]


class CheckPathsTests(unittest.TestCase):
    def test_flake_inputs_do_not_hide_missing_files(self):
        with tempfile.TemporaryDirectory() as directory:
            document = Path(directory) / "README.md"
            document.write_text(
                "`nix flake update config/keycloak2aws`\n"
                "```bash\n"
                "nix flake update config/keycloak2aws config/nixpkgs\n"
                "nix flake update config/keycloak2aws --flake config/missing-flake\n"
                "```\n"
                "`config/missing-file.nix`\n"
                "[Missing file](config/missing-link.nix)\n"
            )
            errors = CHECK_DOCUMENT(document)
            self.assertEqual(len(errors), 3, errors)
            for error, path in zip(
                errors,
                [
                    "config/missing-flake",
                    "config/missing-file.nix",
                    "config/missing-link.nix",
                ],
            ):
                self.assertTrue(error.endswith(f"missing path: {path}"), error)


if __name__ == "__main__":
    unittest.main()
