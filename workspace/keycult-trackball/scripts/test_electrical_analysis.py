# /// script
# requires-python = ">=3.11"
# dependencies = ["sexpdata==1.0.2"]
# ///
"""Check geometry accounting and the boundaries of desk-review calculations."""
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

import sexpdata

from analyze_electrical import microstrip_ohms, sensor_review, thermal, usb_geometry
from usb_routing import apply_usb_route


class ElectricalReviewTests(unittest.TestCase):
    def test_geometry_keeps_layers_separate_and_excludes_via_barrels(self):
        board = sexpdata.loads('''(kicad_pcb
          (segment (start 0 0) (end 3 4) (width 0.2) (layer "In2.Cu") (net "USB_DM"))
          (segment (start 3 4) (end 6 4) (width 0.15) (layer "B.Cu") (net "USB_DM"))
          (via (at 3 4) (net "USB_DM"))
          (via (at 3 6) (net "GND"))
          (segment (start 0 0) (end 0 1) (width 0.2) (layer "In2.Cu") (net "USB_DP"))
          (segment (start 0 0) (end 0 1) (width 0.15) (layer "B.Cu") (net "USB_DM_MCU"))
          (segment (start 0 0) (end 0 1) (width 0.15) (layer "B.Cu") (net "USB_DP_MCU")))''')
        report = usb_geometry(board)["USB_DM"]
        self.assertEqual(report["segment_sum_mm"], 8)
        self.assertEqual(report["layer_length_mm"], {"B.Cu": 3, "In2.Cu": 5})
        self.assertEqual(report["layer_widths_mm"], {"B.Cu": [0.15], "In2.Cu": [0.2]})
        self.assertEqual(report["vias"][0]["nearest_ground_via_mm"], 2)

    def test_missing_usb_net_does_not_create_zero_length_success(self):
        with self.assertRaisesRegex(ValueError, "Expected four USB nets"):
            usb_geometry(sexpdata.loads("(kicad_pcb)"))

    def test_thermal_screen_includes_ground_current_and_ambient(self):
        self.assertEqual(thermal(150, 50)["loss_w"], 0.3355)
        self.assertEqual(thermal(150, 50)["estimated_junction_c"], 111.73)
        self.assertGreater(thermal(200, 50)["estimated_junction_c"], 125)
        self.assertEqual(thermal(150, 60)["estimated_junction_c"] - thermal(150, 50)["estimated_junction_c"], 10)

    def test_sensor_loads_keep_startup_and_downstream_capacitance_separate(self):
        parts = {"J2": {"nets": {"1": "SENSOR_SCLK_3V3", "2": "SENSOR_CS_3V3", "3": "GND",
                                  "4": "+3V3", "5": "GND", "6": "SENSOR_MOSI_3V3", "7": "SENSOR_MISO_3V3", "MP": "GND"}}}
        report = sensor_review(parts)
        scenarios = report["steady_load_scenarios"]
        self.assertEqual([row["total_3v3_allocation_ma"] for row in scenarios], [150, 160, 180])
        self.assertEqual([row["input_before_daughterboard_leakage_ma"] for row in scenarios], [151, 161, 181])
        self.assertEqual([row["thermal_at_50c"]["estimated_junction_c"] for row in scenarios], [111.73, 115.78, 123.88])
        startup = report["startup_current_screen"]
        self.assertEqual(startup["conservative_pmw_ramp_sum_ma"], 130)
        self.assertEqual(startup["host_plus_pmw_ramp_ma"], 240)
        caps = report["external_module_nominal_capacitance_uf"]
        self.assertEqual(caps["connector_input_bulk"], 10)
        self.assertEqual(caps["regulated_downstream_bulk"], 20)

    def test_sensor_connector_rejects_5v_power_and_swapped_spi(self):
        for contact, wrong_net in (("4", "VBUS_FUSED"), ("4", "GND"),
                                   ("3", "+3V3"), ("7", "SENSOR_MOSI_3V3"), ("MP", "+3V3")):
            with self.subTest(contact=contact, wrong_net=wrong_net):
                nets = {"1": "SENSOR_SCLK_3V3", "2": "SENSOR_CS_3V3", "3": "GND", "4": "+3V3",
                        "5": "GND", "6": "SENSOR_MOSI_3V3", "7": "SENSOR_MISO_3V3", "MP": "GND"}
                nets[contact] = wrong_net
                with self.assertRaisesRegex(ValueError, "J2 sensor contact map changed"):
                    sensor_review({"J2": {"nets": nets}})

    def test_logical_pad_variant_does_not_claim_sensor_power_integration(self):
        report = sensor_review({})
        self.assertIn("no module power or connector", report["status"])
        self.assertNotIn("steady_load_scenarios", report)

    def test_microstrip_candidate_and_invalid_dimensions(self):
        self.assertAlmostEqual(microstrip_ohms(0.29, 0.2, 0.2104, 0.035, 4.4), 90.34, places=2)
        self.assertGreater(microstrip_ohms(0.2, 0.2, 0.2104, 0.035, 4.4), 100)
        with self.assertRaises(ValueError):
            microstrip_ohms(0.2, 0.2, 0, 0.035, 4.4)

    def test_usb_replacement_preserves_other_copper_and_is_repeatable(self):
        retained = sexpdata.loads('''(segment (start 1 2) (end 3 4)
          (width .15) (layer "B.Cu") (net "USB_DM_MCU") (uuid "retained"))''')
        with TemporaryDirectory() as directory:
            path = Path(directory) / 'board.kicad_pcb'
            path.write_text('''(kicad_pcb (setup)
              (segment (start 0 0) (end 3 4) (width .2) (layer "In2.Cu") (net "USB_DM"))
              (via (at 3 4) (net "USB_DM"))
              (via (at 4 4) (net "GND") (uuid "existing-ground"))
              ''' + sexpdata.dumps(retained) + ')')
            apply_usb_route(path)
            first = path.read_text()
            apply_usb_route(path)
            self.assertEqual(path.read_text(), first)
            board = sexpdata.loads(first)
            self.assertIn(retained, board)
            self.assertIn(sexpdata.loads('(via (at 4 4) (net "GND") (uuid "existing-ground"))'), board)
            self.assertNotIn('"In2.Cu") (net "USB_DM")', first)


    def test_top_connector_preserves_short_route_and_only_sets_stackup(self):
        original = sexpdata.loads('''(kicad_pcb (setup)
          (footprint "JST" (layer "F.Cu") (property "Reference" "J1"))
          (segment (start 152 64) (end 155 64) (width .29) (layer "F.Cu") (net "USB_DM"))
          (via (at 151 64) (net "GND")))''')
        with TemporaryDirectory() as directory:
            path = Path(directory) / 'board.kicad_pcb'
            path.write_text(sexpdata.dumps(original))
            apply_usb_route(path)
            first = path.read_text()
            board = sexpdata.loads(first)
            self.assertEqual(board[2:], original[2:])
            self.assertEqual(str(board[1][1][0]), 'stackup')
            apply_usb_route(path)
            self.assertEqual(path.read_text(), first)

    def test_usb_stitches_replace_imported_uuids_without_removing_other_vias(self):
        stitches = [sexpdata.loads(f'''(via (at {x} {y}) (size .6) (drill .3)
          (layers "F.Cu" "B.Cu") (net "GND") (uuid "imported-{i}"))''')
          for i, (x, y) in enumerate([(38.5, 113.2), (113.4, 55.7), (118.5, 54.7)])]
        retained = [sexpdata.loads(f'''(via (at 38.5 113.2) (size {size}) (drill {drill})
          (layers "F.Cu" "{layer}") (net "{net}") (uuid "other-{i}"))''')
          for i, (size, drill, layer, net) in enumerate([
              (.6, .3, 'B.Cu', '+3V3'), (.8, .3, 'B.Cu', 'GND'),
              (.6, .4, 'B.Cu', 'GND'), (.6, .3, 'In1.Cu', 'GND')])]
        with TemporaryDirectory() as directory:
            path = Path(directory) / 'board.kicad_pcb'
            path.write_text(sexpdata.dumps(sexpdata.loads('(kicad_pcb (setup))') + stitches + retained))
            apply_usb_route(path)
            first = path.read_text()
            board = sexpdata.loads(first)
            for via in stitches:
                self.assertNotIn(via, board)
                self.assertEqual(sum(isinstance(item, list) and item[:-1] == via[:-1] for item in board), 1)
            for via in retained:
                self.assertIn(via, board)
            apply_usb_route(path)
            self.assertEqual(path.read_text(), first)


if __name__ == "__main__":
    unittest.main()
