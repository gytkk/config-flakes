"""Exercise both inventory builders without downloading their flake dependencies."""
import json
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[2]


class InventoryValidationTests(unittest.TestCase):
    def evaluate(self, builder, config):
        if builder == "home":
            expression = f'''let
              builders = import {ROOT}/config/lib/home-configurations.nix {{
                inputs.home-manager.lib.homeManagerConfiguration = args: {{ inherit (args.extraSpecialArgs) username homeDirectory flakeDirectory; }};
                systemPkgs = {{}};
              }};
              home = if builtins.functionArgs builders.mkHomeConfig ? baseModules
                then builders.mkHomeConfig {{ baseModules = []; }}
                else builders.mkHomeConfig;
            in home "test" ({config})'''
        else:
            expression = f'''let
              builders = import {ROOT}/config/lib/nixos-configurations.nix {{
                inputs = {{}};
                nixpkgs.lib.nixosSystem = args: {{ inherit (args.specialArgs) username homeDirectory flakeDirectory; }};
                commonOverlays = [];
                mkFlakeDirectory = home: home + "/ws";
              }};
            in builders.mkNixOSConfig "test" ({config})'''
        return subprocess.run(
            ["nix", "eval", "--impure", "--json", "--expr", expression],
            capture_output=True, text=True,
        )

    def test_missing_fields_fail_before_resolving_builder_dependencies(self):
        for builder, label in [("home", "test"), ("nixos", "NixOS host test")]:
            with self.subTest(builder=builder):
                result = self.evaluate(builder, '{ system = "unknown"; }')
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(
                    f"Missing required fields for {label}: username homeDirectory profile",
                    result.stderr,
                )

    def test_real_inventory_entries_preserve_checkout_and_identity(self):
        inventory = json.loads(subprocess.check_output(
            ["nix", "eval", "--impure", "--json", "--expr",
             f"builtins.attrNames (import {ROOT}/config/inventory.nix)"], text=True,
        ))
        for name in inventory:
            config = f'(import {ROOT}/config/inventory.nix)."{name}"'
            for builder in ["home", "nixos"]:
                with self.subTest(host=name, builder=builder):
                    # Select only public arguments; do not force actual module evaluation.
                    result = self.evaluate(builder, config)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    actual = json.loads(result.stdout)
                    self.assertEqual(actual["username"], "gyutak" if name.startswith("devsisters") else "gytkk")
                    self.assertEqual(actual["flakeDirectory"], actual["homeDirectory"] + "/ws")
