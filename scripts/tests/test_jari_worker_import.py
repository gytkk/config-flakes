"""Exercise the import a channel-based worker loads, without fetching flake inputs."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


class WorkerImportTest(unittest.TestCase):
    def test_import_keeps_nested_sources_and_needs_no_checkout(self):
        with tempfile.TemporaryDirectory() as tmp:
            checkout = Path(tmp).resolve() / "flake"
            checkout.mkdir()
            (checkout / "flake.nix").write_text(
                '{ outputs = { self }: { nixosModules.devsisters-jari.answer = 42; }; }'
            )
            expression = f'''let
              flake = builtins.getFlake "path:{checkout}";
              leaf.outPath = builtins.toFile "jari-test-private-input" "locked source";
              dep = {{ outPath = builtins.toFile "jari-test-input" "source"; inputs = {{ inherit leaf; }}; }};
              module = import {ROOT}/systems/base/jari/worker-import.nix
                (flake // {{ inputs = {{ inherit dep; }}; }});
            in [module flake.outPath dep.outPath leaf.outPath]'''
            module, *sources = json.loads(subprocess.check_output(
                ["nix", "eval", "--offline", "--impure", "--json", "--expr", expression], text=True,
            ))
        refs = subprocess.check_output(["nix-store", "-q", "--references", module], text=True).splitlines()
        self.assertEqual(set(refs), set(sources))
        answer = subprocess.check_output(
            ["nix", "eval", "--offline", "--impure", "--json", "--expr", f"(import {module}).answer"], text=True,
        )
        self.assertEqual(json.loads(answer), 42)
