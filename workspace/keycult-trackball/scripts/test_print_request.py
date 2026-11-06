# /// script
# requires-python = ">=3.11"
# ///
"""Exercise bench-fixture geometry gates with deliberate defects."""
import json
import unittest

from build_print_request import ROOT, V, box, check_fixtures, cylinder, fixtures, geometry


class FixtureChecks(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.layout = json.loads((ROOT/'design/preliminary/layout.json').read_text())['rear_clearance']
        cls.holder, cls.refs = geometry()
        cls.parts, cls.keys = fixtures(cls.layout)

    def verify(self, parts):
        return check_fixtures(parts, self.holder, self.refs, self.keys, self.layout['pcb_opening'])

    def test_current_geometry(self):
        result = self.verify(self.parts)
        self.assertLess(result['max_overlap_mm3'], 1e-6)
        self.assertEqual(len(result['switch_centers']), 10)
        self.assertEqual(json.loads(json.dumps(result))['gauge_opening'], self.layout['pcb_opening'])

    def test_blocked_switch_opening(self):
        parts = self.parts.copy()
        k = self.keys[0]
        parts['BenchPlate'] = parts['BenchPlate'].fuse(
            box(k['x_mm']-7, k['y_mm']-7, -1.5, 14, 14, 1.5)).removeSplitter()
        with self.assertRaisesRegex(AssertionError, 'Switch opening blocked'):
            self.verify(parts)

    def test_blocked_holder_window(self):
        parts = self.parts.copy()
        parts['BenchPlate'] = parts['BenchPlate'].fuse(box(-18, -18, -1.5, 36, 36, 1.5)).removeSplitter()
        with self.assertRaises(AssertionError):
            self.verify(parts)

    def test_missing_support(self):
        parts = self.parts.copy()
        parts['BenchStand'] = parts['BenchStand'].copy()
        parts['BenchStand'].translate(V(0, 0, -3))
        with self.assertRaisesRegex(AssertionError, 'Missing plate support'):
            self.verify(parts)

    def test_wrong_spacer_height(self):
        parts = self.parts.copy()
        parts['SensorSpacer3p75mm'] = cylinder(2.2, 0, 4).cut(cylinder(1.15, -1, 6))
        with self.assertRaises(AssertionError):
            self.verify(parts)


if __name__ == '__main__':
    unittest.main()
