# /// script
# requires-python = ">=3.11"
# dependencies = ["shapely==2.1.2"]
# ///
"""Regression-test the real final validator gate without running KiCad."""
import ast
from collections import Counter
import csv
import hashlib
import json
import io
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
import xml.etree.ElementTree as ET
from shapely.geometry import box, Point

SOURCE = Path(__file__).with_name("validate_pcb.py")
GATE = compile(ast.Module(body=[ast.parse(SOURCE.read_text()).body[-1]], type_ignores=[]), str(SOURCE), "exec")
VARIANT_CODE = compile(ast.Module(body=[node for node in ast.parse(SOURCE.read_text()).body if (isinstance(node, ast.FunctionDef) and node.name in {'indexed_keys', 'circuit_net_matches', 'reservation_matches', 'usb_connector_matches', 'usb_uncovered_area', 'sensor_connector_matches', 'outline_matches', 'stackup_layers', 'stackup_matches', 'placement_rows'}) or (isinstance(node, ast.Assign) and any(isinstance(target, ast.Name) and target.id == 'VARIANTS' for target in node.targets))], type_ignores=[]), str(SOURCE), "exec")

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

class CleanupGeometryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        source = SOURCE.with_name('cleanup_pcb_routes.py')
        functions = [node for node in ast.parse(source.read_text()).body if isinstance(node, ast.FunctionDef) and node.name in {'tag', 'child', 'junction_points', 'junction_candidates', 'unconnected_per_net'}]
        cls.namespace = {'Counter': Counter, 're': re}
        exec(compile(ast.Module(body=functions, type_ignores=[]), str(source), 'exec'), cls.namespace)

    def test_dangling_tail_is_separated_from_a_live_t_junction(self):
        split = self.namespace['junction_points']
        self.assertEqual(split((50, 50), (60, 50), [(55, 50)]), [(50, 50), (55, 50), (60, 50)])
        self.assertEqual(split((60, 50), (50, 50), [(55, 50)]), [(60, 50), (55, 50), (50, 50)])

    def test_junction_accepts_dsn_rounding_and_rejects_another_trace(self):
        split = self.namespace['junction_points']
        self.assertEqual(split((0, 0), (10, 0), [(5, .0001), (6, .01), (11, 0)]), [(0, 0), (5, .0001), (10, 0)])

    def test_only_same_net_same_layer_connections_split_a_track(self):
        def track(net, layer, start, end):
            return ['segment', ['net', net], ['layer', layer], ['start', *start], ['end', *end]]
        main = track('ROW0', 'F.Cu', (0, 0), (10, 0))
        others = [main, track('ROW0', 'F.Cu', (5, 0), (5, 5)), track('COL1', 'F.Cu', (3, 0), (3, 5)), track('ROW0', 'B.Cu', (7, 0), (7, 5)), track('ROW0', 'F.Cu', (10, 0), (15, 0))]
        points = self.namespace['junction_candidates'](main, others)
        self.assertEqual(self.namespace['junction_points']((0, 0), (10, 0), points), [(0, 0), (5, 0), (10, 0)])

    def test_disconnect_guard_counts_each_net_once_per_finding(self):
        report = {'unconnected_items': [{'items': [{'description': 'Pad [ROW0]'}, {'description': 'Track [ROW0]'}]}, {'items': [{'description': 'Pad [COL1]'}]}]}
        self.assertEqual(self.namespace['unconnected_per_net'](report), Counter({'ROW0': 1, 'COL1': 1}))

    @unittest.skipUnless(shutil.which('kicad-cli'), 'KiCad CLI unavailable')
    def test_native_cleanup_removes_only_the_unused_t_junction_tail(self):
        fixture = '(kicad_pcb (version 20260206) (generator "pcbnew") (general (thickness 1.6)) (layers (0 "F.Cu" signal) (2 "B.Cu" signal) (5 "F.SilkS" user) (25 "Edge.Cuts" user))\n'
        for index, (x, y) in enumerate(((50, 50), (55, 55)), 1):
            fixture += f'(footprint "" (layer "F.Cu") (at {x} {y}) (property "Reference" "TP{index}" (at 0 -1 0) (layer "F.SilkS") (effects (font (size 1 1)))) (attr smd) (pad "1" smd circle (at 0 0) (size 1 1) (layers "F.Cu") (net "SIGNAL")))\n'
        for x1, y1, x2, y2 in ((50, 50, 60, 50), (55, 50, 55, 55)):
            fixture += f'(segment (start {x1} {y1}) (end {x2} {y2}) (width .25) (layer "F.Cu") (net "SIGNAL"))\n'
        for x1, y1, x2, y2 in ((40, 40, 70, 40), (70, 40, 70, 70), (70, 70, 40, 70), (40, 70, 40, 40)):
            fixture += f'(gr_line (start {x1} {y1}) (end {x2} {y2}) (stroke (width .05) (type default)) (layer "Edge.Cuts"))\n'
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            board = root / 'design/pcb/62/keycult62.kicad_pcb'
            board.parent.mkdir(parents=True)
            board.write_text(fixture + ')\n')
            script = root / 'scripts/cleanup_pcb_routes.py'
            script.parent.mkdir()
            shutil.copyfile(SOURCE.with_name('cleanup_pcb_routes.py'), script)
            reports = root / '.cache/pcb-tools'
            reports.mkdir(parents=True)
            before = reports / 'before.json'
            result = subprocess.run(['kicad-cli', 'pcb', 'drc', '--save-board', '--format', 'json', '-o', str(before), str(board)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            report = json.loads(before.read_text())
            self.assertEqual(report['unconnected_items'], [])
            self.assertEqual([finding['type'] for finding in report['violations']], ['track_dangling'])
            result = subprocess.run(['uv', 'run', '--python', sys.executable, str(script), '--keys', '62'], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            report = json.loads((reports / 'cleanup-drc-62.json').read_text())
            self.assertEqual(report['violations'], [])
            self.assertEqual(report['unconnected_items'], [])

class VariantTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.namespace = {'re': re, 'csv': csv, 'io': io}
        exec(VARIANT_CODE, cls.namespace)
        cls.layout = json.loads((SOURCE.parents[1] / 'design/preliminary/layout.json').read_text())

    def test_stackup_requires_all_four_copper_layers_in_order(self):
        layers = self.namespace['stackup_layers']((SOURCE.parents[1] / 'design/pcb/62/keycult62.kicad_pcb').read_text())
        matches = self.namespace['stackup_matches']
        self.assertTrue(matches(layers))
        self.assertFalse(matches(layers[:-1]))
        self.assertFalse(matches(list(reversed(layers))))
        self.assertFalse(matches([('F.Cu', 'copper', .07, None, None), *layers[1:]]))
        self.assertFalse(matches([layers[0], (*layers[1][:4], 4.9), *layers[2:]]))
        self.assertAlmostEqual(sum(layer[2] for layer in layers), 1.5862)

    def test_placement_checks_origin_side_and_rotation_and_omits_testpads(self):
        content = 'Ref,Val,Package,PosX,PosY,Rot,Side\nU1,RP2040,QFN,100,-70,90,bottom\nTP1,GND,pad,50,-55,0,bottom\nSW1,MX,MX,40,-60,0,top\n'
        expected = {'U1': (100, 70, 90, 'bottom'), 'TP1': (50, 55, 0, 'bottom'), 'SW1': (40, 60, 0, 'top')}
        rows = self.namespace['placement_rows']
        self.assertEqual([row['Ref'] for row in rows(content, expected)], ['U1'])
        for bad in (content.replace('100,-70', '-100,-70'), content.replace('100,-70', '100,70'), content.replace('90,bottom', '0,bottom'), content.replace('90,bottom', '90,top'), content.replace('U1,RP2040', 'DB1,RP2040'), content + 'U1,RP2040,QFN,100,-70,90,bottom\n'):
            with self.subTest(content=bad):
                with self.assertRaises(ValueError): rows(bad, expected)

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

    def test_shallow_reservation_follows_opening_with_quarter_mm_guard(self):
        self.assertEqual(self.namespace['VARIANTS'][65][1], [155, 75, 205, 125])
        self.assertEqual(self.namespace['VARIANTS'][62][1], [155.75, 75.75, 204.25, 124.25])
        x1, y1, x2, y2 = self.namespace['VARIANTS'][62][1]
        self.assertAlmostEqual(x2 - x1, 48.5)
        self.assertEqual(y1, 75.75)
        self.assertEqual([x1-180,y1-100,x2-180,y2-100], self.layout['rear_clearance']['pcb_keepout_mm'])

    def test_sensor_supply_and_numbering_are_independent_of_usb(self):
        matches = self.namespace['sensor_connector_matches']
        pins = {'1':'SENSOR_SCLK_3V3','2':'SENSOR_CS_3V3','3':'GND','4':'+3V3','5':'GND','6':'SENSOR_MOSI_3V3','7':'SENSOR_MISO_3V3','MP':'GND'}
        self.assertTrue(matches(pins))
        for bad in ({**pins,'4':'VBUS_FUSED'}, {**pins,'6':pins['7'],'7':pins['6']}, {**pins,'MP':'+3V3'}):
            self.assertFalse(matches(bad))

    def test_shallow_variant_places_all_smt_on_top_and_keeps_switch_centres(self):
        directory = SOURCE.parents[1] / 'design/pcb/62'
        parts = {p['ref']: p for p in json.loads((directory / 'circuit.json').read_text())['parts']}
        for ref, part in parts.items():
            if part['fp'] and not ref.startswith('TP'):
                self.assertEqual(part['side'], 'F', ref)
        for index, key in self.namespace['indexed_keys'](self.layout, 'rear_clearance'):
            for actual, target in zip(parts[f'SW{index}']['pos'], [182.54 + key['x_mm'], 94.92 + key['y_mm']]):
                self.assertAlmostEqual(actual, target)
            expected = (180 + key['x_mm'] + (4 if key['row'] == 4 else -4), 100 + key['y_mm'] + (-8.9 if key['row'] == 4 else 8.9))
            for actual, target in zip(parts[f'D{index}']['pos'], expected):
                self.assertAlmostEqual(actual, target)
        self.assertEqual(parts['J1']['pos'], [153, 64])
        self.assertEqual(parts['J2']['pos'], [177, 58])

    def test_controller_seed_provenance_and_record_domain(self):
        root = SOURCE.parents[1]
        seed = json.loads((root / 'design/pcb/62/controller-seed.json').read_text())
        self.assertEqual(seed['source_sha256'], hashlib.sha256((root / seed['source_pcb']).read_bytes()).hexdigest())
        parts = json.loads((root / 'design/pcb/62/circuit.json').read_text())['parts']
        nets = {net for part in parts for net in part['nets'].values() if net is not None}
        self.assertGreater(len(seed['records']), 500)
        for record in seed['records']:
            self.assertIn(record['net'], nets)
            self.assertNotEqual(record['net'], 'COL7')
            self.assertFalse(record['net'].startswith('K_'))
            self.assertIn(record['kind'], ('segment', 'via'))
            points = [record['at']] if record['kind'] == 'via' else [record['start'], record['end']]
            if record['kind'] == 'segment':
                self.assertIn(record['layer'], ('F.Cu', 'B.Cu', 'In1.Cu', 'In2.Cu'))
                self.assertNotEqual(record['start'], record['end'])
            for point in points:
                for value in point: self.assertAlmostEqual(value, round(value, 4))

    def test_native_opening_rejects_missing_shifted_or_sharp_edges(self):
        import re
        native = (SOURCE.parents[1] / 'design/pcb/62/keycult62.kicad_pcb').read_text()
        # Native records can be compact or formatted; each edge ends at its UUID.
        records = re.findall(r'\(gr_(line|arc)\s+(.*?)\(uuid\s+"[^"]+"\)\s*\)', native, re.S)
        edges = []
        for kind, record in records:
            if '(layer "Edge.Cuts")' not in record:
                continue
            edge = {}
            for key in ('start','end','mid'):
                match = re.search(r'\('+key+r'\s+([-\d.]+)\s+([-\d.]+)\)',record)
                if match:edge[key] = tuple(map(float,match.groups()))
            edges.append(edge)
        matches = self.namespace['outline_matches']
        self.assertTrue(matches(edges,62))
        self.assertFalse(matches(edges,65))
        self.assertFalse(matches(edges[:-1],62))
        self.assertFalse(matches([{**e,'start':(e['start'][0]+.1,e['start'][1])} for e in edges],62))
        self.assertFalse(matches([{k:v for k,v in e.items() if k!='mid'} for e in edges],62))
        opening = self.layout['rear_clearance']['pcb_opening']
        self.assertEqual(opening['bounds_mm'], [-24, -24, 24, 24])
        self.assertEqual(opening['corner_radius_mm'], 1)
        # Restore DRAFT F's 48x60 opening without changing the outer board edges.
        old_edges = [{key: (x, y - 11 if y < 100 else y + 1) for key, (x, y) in edge.items()} if 155 < edge['start'][0] < 205 and 155 < edge['end'][0] < 205 else edge for edge in edges]
        self.assertFalse(matches(old_edges,62))

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
