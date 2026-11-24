import runpy
import tempfile
import unittest
from pathlib import Path


document = runpy.run_path(str(Path(__file__).parents[2] / "pi/files/session-editor.py"))["document"]


class DocumentTargetTest(unittest.TestCase):
    def test_decodes_the_path_and_resolves_a_heading_or_line(self):
        with tempfile.TemporaryDirectory() as root:
            cwd = Path(root).resolve()
            path = cwd / "Pi notes.md"
            path.write_text("# Setup\n\n## Hindsight memory\n")
            self.assertEqual(document("Pi%20notes.md#hindsight-memory", cwd), (path, 3))
            self.assertEqual(document(path.as_uri() + "#L2", cwd), (path, 2))
            self.assertEqual(document("Pi%20notes.md#missing", cwd), (path, 1))

    def test_does_not_follow_links_out_of_the_clicked_checkout(self):
        with tempfile.TemporaryDirectory() as root:
            cwd = Path(root).resolve() / "checkout"
            cwd.mkdir()
            outside = cwd.parent / "private.md"
            outside.write_text("# Private")
            (cwd / "link.md").symlink_to(outside)
            for target in ["../private.md", "link.md", "file://elsewhere/private.md", "https://example.org/x.md", "%00.md"]:
                self.assertIsNone(document(target, cwd))
