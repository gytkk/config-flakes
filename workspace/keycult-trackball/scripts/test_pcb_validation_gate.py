# /// script
# requires-python = ">=3.11"
# dependencies = ["shapely==2.1.2"]
# ///
"""Regression-test the real final validator gate without running KiCad."""
import ast
import csv
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
import xml.etree.ElementTree as ET
from shapely.geometry import box, Point

SOURCE = Path(__file__).with_name("validate_pcb.py")
GATE = compile(ast.Module(body=[ast.parse(SOURCE.read_text()).body[-1]], type_ignores=[]), str(SOURCE), "exec")
VARIANT_CODE = compile(ast.Module(body=[node for node in ast.parse(SOURCE.read_text()).body if (isinstance(node, ast.FunctionDef) and node.name in {'indexed_keys', 'circuit_net_matches', 'reservation_matches', 'usb_connector_matches', 'usb_uncovered_area'}) or (isinstance(node, ast.Assign) and any(isinstance(target, ast.Name) and target.id == 'VARIANTS' for target in node.targets))], type_ignores=[]), str(SOURCE), "exec")

class DraftGateTests(unittest.TestCase):
    def result(self, draft=True, errs=None, viol=None, unconn=None):
        try:
            exec(GATE, {"args": SimpleNamespace(draft=draft), "errs": errs or [], "viol": viol or [], "unconn": unconn or []})
        except SystemExit as error:
            return error.code
        return 0

    def test_clean_passes(self):
        self.assertEqual(self.result(), 0)
        self.assertEqual(self.result(draft=False), 0)

    def test_only_draft_permits_unconnected(self):
        self.assertEqual(self.result(unconn=[{}]), 0)
        self.assertEqual(self.result(draft=False, unconn=[{}]), 1)

    def test_drc_always_fails(self):
        for draft in (True, False):
            for kind in ("shorting_items", "clearance"):
                with self.subTest(draft=draft, kind=kind):
                    self.assertEqual(self.result(draft=draft, viol=[{"type": kind}]), 1)

    def test_structural_error_always_fails(self):
        for draft in (True, False):
            self.assertEqual(self.result(draft=draft, errs=["net mismatch"]), 1)

class VariantTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.namespace = {}
        exec(VARIANT_CODE, cls.namespace)
        cls.layout = json.loads((SOURCE.parents[1] / 'design/preliminary/layout.json').read_text())

    def test_variant_preserves_original_references_and_coordinates(self):
        for count, (variant, _) in self.namespace['VARIANTS'].items():
            with self.subTest(keys=count):
                keys = self.namespace['indexed_keys'](self.layout, variant)
                self.assertEqual(len(keys), count)
                self.assertEqual([key for _, key in keys], self.layout[variant]['keys'])
                self.assertEqual({index for index, _ in keys}, set(range(1, 67)) - {59} - ({7, 8, 9} if count == 62 else set()))
                bottom = {index: key for index, key in keys if key['row'] == 4 and key['group'] == 'center'}
                self.assertEqual(set(bottom), {58, 60})
                for index, label, x in [(58, 'M1', -14.2875), (60, 'M2', 14.2875)]:
                    self.assertEqual((bottom[index]['label'], bottom[index]['x_mm'], bottom[index]['y_mm'], bottom[index]['width_u']), (label, x, 38.1, 1.5))

    def test_rear_clearance_reservation_is_clipped_to_board(self):
        self.assertEqual(self.namespace['VARIANTS'][65][1], [155, 75, 205, 125])
        self.assertEqual(self.namespace['VARIANTS'][62][1], [151.425, 52.7, 208.575, 128.575])
        x1, y1, x2, y2 = self.namespace['VARIANTS'][62][1]
        self.assertAlmostEqual(x2 - x1, 57.15)
        self.assertEqual(y1, 52.7)
        self.assertAlmostEqual(y2, 100 + 28.575)

    def test_no_connect_requires_own_single_pad_net(self):
        matches = self.namespace['circuit_net_matches']
        for ref, pin, net in [('U1', '37', 'unconnected-(U1-GPIO25-Pad37)'), ('U1', '39', 'unconnected-(U1-GPIO27{slash}ADC1-Pad39)'), ('U3', '4', 'unconnected-(U3-NC-Pad4)')]:
            with self.subTest(ref=ref, pin=pin):
                self.assertTrue(matches(None, net, ref, pin, 1))
                self.assertFalse(matches(None, net, ref, pin, 2))
                self.assertFalse(matches(None, net, 'U99', pin, 1))
                self.assertFalse(matches(None, net, ref, '99', 1))
        for net in ('', 'GND', '+3V3'):
            self.assertFalse(matches(None, net, 'U1', '37', 1))
        self.assertTrue(matches('QSPI_SD0', 'QSPI_SD0', 'U1', '53', 2))
        self.assertFalse(matches('QSPI_SD0', 'GND', 'U1', '53', 2))

    def test_usb_connector_rejects_reversed_cable_and_testpad_numbering(self):
        matches = self.namespace['usb_connector_matches']
        self.assertTrue(matches({'1':'VBUS_USB','2':'USB_DM','3':'USB_DP','4':'GND','MP':'GND'}))
        self.assertFalse(matches({'1':'GND','2':'USB_DP','3':'USB_DM','4':'VBUS_USB','MP':'GND'}))
        self.assertFalse(matches({'1':'VBUS_USB','2':'GND','3':'USB_DM','4':'USB_DP','MP':'GND'}))
        self.assertFalse(matches({'1':'VBUS_USB','2':'USB_DP','3':'USB_DM','4':'GND','MP':'GND'}))

    def test_usb_reference_hole_outside_via_transition_is_not_excluded(self):
        uncovered = self.namespace['usb_uncovered_area']
        trace = box(0, -.145, 10, .145)
        plane = box(-1, -1, 11, 1)
        antipad = Point(0, 0).buffer(.5)
        transitions = Point(0, 0).buffer(.51)
        reference = plane.difference(antipad)
        self.assertEqual(uncovered(trace, reference, transitions), 0)
        self.assertGreater(uncovered(trace, reference, Point().buffer(0)), 0)
        self.assertGreater(uncovered(trace, reference.difference(box(5, -.1, 5.2, .1)), transitions), .039)

    def test_external_c5_jsh_assembly_and_retained_main_board_protection(self):
        for count in (65, 62):
            with self.subTest(keys=count):
                directory = SOURCE.parents[1] / 'design/pcb' / ('62' if count == 62 else '')
                parts = {p['ref']: p for p in json.loads((directory / 'circuit.json').read_text())['parts']}
                expected = {'1': 'VBUS_USB', '2': 'USB_DM', '3': 'USB_DP', '4': 'GND'}
                self.assertEqual(parts['DB1']['nets'], expected)
                self.assertEqual(parts['DB1']['value'], 'UDB C5-JSH / EXTERNAL')
                self.assertEqual(parts['DB1']['fp'], '')
                self.assertEqual(parts['U1']['nets']['27'], None if count == 62 else 'COL7')
                self.assertEqual(parts['F1']['nets'], {'1': 'VBUS_USB', '2': 'VBUS_FUSED'})
                self.assertEqual(parts['F1']['mpn'], 'Bourns MF-PSMF050X-2')
                for ref in ('R1', 'R2'):
                    self.assertEqual(parts[ref]['value'], '27R')
                with (directory / 'bom.csv').open() as file:
                    bom = {row['Reference']: row for row in csv.DictReader(file)}
                self.assertEqual(bom['DB1']['Assembly'], 'External assembly')
                netlist = ET.parse(directory / 'reports/netlist.xml').getroot()
                actual = {n.attrib['pin']: net.attrib['name'] for net in netlist.find('nets') for n in net.findall('node') if n.attrib['ref'] == 'DB1'}
                self.assertEqual(actual, expected)

    def test_reservation_accepts_native_rounding_but_rejects_geometry_changes(self):
        matches = self.namespace['reservation_matches']
        reservation = box(151.425, 52.7, 208.575, 128.575)
        self.assertTrue(matches(box(151.425, 52.7, 208.575, 128.574999), reservation))
        self.assertFalse(matches(box(151.425, 52.7, 208.575, 128.5749), reservation))
        self.assertFalse(matches(box(155, 75, 205, 125), reservation))
        self.assertFalse(matches(reservation.difference(box(170, 90, 190, 110)), reservation))

if __name__ == "__main__":
    unittest.main()
