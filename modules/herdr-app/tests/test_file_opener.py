import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import unittest
from urllib.parse import quote


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("file_opener", ROOT / "file-opener.py")
opener = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(opener)


class FileLinks(unittest.TestCase):
    def link(self, path, suffix=""):
        return f"omp-nvim://{opener.host_id()}.p123{quote(str(path))}{suffix}"

    def test_decodes_literal_paths_and_location_without_treating_them_as_commands(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "한글 ' $HOME | +file.lua"
            path.touch()
            self.assertEqual(opener.parse_link(self.link(path, "?line=42&col=7")), ("p123", path.resolve(), 42, 7))
            self.assertEqual(opener.parse_link(self.link(path, "#L9C3"))[2:], (9, 3))
            self.assertEqual(opener.parse_link(self.link(path, "?line=2#L9"))[2], 2)

    def test_rejects_foreign_hosts_malformed_paths_and_ambiguous_locations(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "code.py"
            path.touch()
            valid = self.link(path)
            foreign = "f" * 16 if opener.host_id() != "f" * 16 else "e" * 16
            for link in (
                valid.replace(opener.host_id(), foreign), valid.replace("omp-nvim:", "file:"),
                valid.replace(".p123", ".p123:80"), valid + "%ZZ", valid + "%00",
                valid + "?line=1&line=2", valid + "?line=-1", valid + "?col=2147483648",
            ):
                with self.subTest(link=link), self.assertRaises(ValueError):
                    opener.parse_link(link)

    @unittest.skipUnless(Path("/usr/bin/file").exists(), "file is not installed")
    def test_uses_content_instead_of_extension_to_keep_binary_files_out_of_nvim(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            files = {
                "Makefile": (b"all:\n\ttrue\n", True),
                "settings.json": (b'{"enabled":true}\n', True),
                "empty": (b"", True),
                "fake-source.py": (b"\x00\x01\x02\x03" * 100, False),
                "document.txt": (b"%PDF-1.4\n1 0 obj\n<</Type /Catalog>>\nendobj\n", False),
            }
            for name, (content, expected) in files.items():
                path = root / name
                path.write_bytes(content)
                with self.subTest(name=name):
                    self.assertEqual(opener.is_text(path), expected)
            self.assertFalse(opener.is_text(root))


@unittest.skipUnless(shutil.which("nvim"), "Neovim is not installed")
class EditorNavigation(unittest.TestCase):
    def test_reuses_editor_preserves_unsaved_buffers_and_clamps_locations(self):
        with tempfile.TemporaryDirectory(prefix="omp-nvim-test-", dir="/tmp") as directory:
            root = Path(directory)
            address = root / "nvim.sock"
            first = root / "first ' | $HOME.lua"
            second = root / "+other file.md"
            first.write_text("one\ntwo\nthree\n")
            second.write_text("alpha\nbeta\n")
            nvim = shutil.which("nvim")
            process = subprocess.Popen([nvim, "--headless", "-u", "NONE", "-i", "NONE", "--listen", str(address)],
                                       stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
            def expr(expression):
                return opener.run([nvim, "--server", str(address), "--remote-expr", expression])
            try:
                deadline = time.monotonic() + 5
                while not address.exists() and process.poll() is None and time.monotonic() < deadline:
                    time.sleep(0.02)
                self.assertTrue(address.exists(), "Neovim server did not start")
                opener.remote(nvim, address, first, 2, 2)
                self.assertEqual(json.loads(expr("json_encode(getcurpos()[1:2])")), [2, 2])
                expr("setline(1, 'unsaved edit')")
                opener.remote(nvim, address, first, 3, 1)
                self.assertEqual(expr("getline(1)"), "unsaved edit")
                opener.remote(nvim, address, second, 999, 999)
                self.assertEqual(json.loads(expr("json_encode(getcurpos()[1:2])")), [2, 4])
                opener.remote(nvim, address, first, 1, 1)
                self.assertEqual(expr("getline(1)"), "unsaved edit")
                self.assertEqual(expr("&modified"), "1")
                self.assertEqual(first.read_text(), "one\ntwo\nthree\n")
                self.assertEqual(expr("expand('%:p')"), str(first))
            finally:
                process.terminate()
                process.communicate(timeout=5)


if __name__ == "__main__":
    unittest.main()
