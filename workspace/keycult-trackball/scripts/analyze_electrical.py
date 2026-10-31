# /// script
# requires-python = ">=3.11"
# dependencies = ["sexpdata==1.0.2"]
# ///
"""Reproduce conditional power estimates and native USB routing measurements."""
import hashlib
import json
import math
from collections import defaultdict
from pathlib import Path

import sexpdata

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "design/pcb/reports/electrical-review.json"


def field(node, name):
    return next((x[1:] for x in node if isinstance(x, list) and x and str(x[0]) == name), None)


def microstrip_ohms(width, gap, height, copper, er):
    """TI SLVA531A equations 1/2; uncoated outer-layer screening only."""
    if min(width, gap, height, copper) <= 0 or er <= 1:
        raise ValueError("Microstrip dimensions must be positive and dielectric constant > 1")
    z0 = 87 / math.sqrt(er + 1.41) * math.log(5.98 * height / (0.8 * width + copper))
    if z0 <= 0:
        raise ValueError("Geometry is outside this approximation's useful range")
    return 2 * z0 * (1 - 0.48 * math.exp(-0.96 * gap / height))


def thermal(load_ma, ambient_c):
    # 1mA ground-current reserve is an assumption, not the no-load IQ limit.
    loss_w = (5.5 - 3.3) * load_ma / 1000 + 5.5 * 0.001
    return {"load_ma": load_ma, "ambient_c": ambient_c,
            "loss_w": round(loss_w, 4), "estimated_junction_c": round(ambient_c + 184 * loss_w, 2)}


def sensor_review(parts):
    if "J2" not in parts:
        return {"status": "MCU-side logical pads only; no module power or connector implemented"}
    expected = {"1": "SENSOR_SCLK_3V3", "2": "SENSOR_CS_3V3", "3": "GND", "4": "+3V3",
                "5": "GND", "6": "SENSOR_MOSI_3V3", "7": "SENSOR_MISO_3V3", "MP": "GND"}
    if parts["J2"]["nets"] != expected:
        raise ValueError("J2 sensor contact map changed; expected +3V3 power and 3.3V SPI")
    return {
        "status": "assumed published Keyball circuit powered from +3V3; hardware revision and operation unverified",
        "source_facts": "../sources.md#keyball-sensor-interface-evidence",
        "connector": "J2",
        "contact_nets": expected,
        "steady_load_scenarios": [
            {"sensor_allocation_ma": sensor_ma, "host_allocation_ma": 110,
             "total_3v3_allocation_ma": 110 + sensor_ma,
             "input_before_daughterboard_leakage_ma": 111 + sensor_ma,
             "thermal_at_50c": thermal(110 + sensor_ma, 50)}
            for sensor_ma in (40, 50, 70)
        ],
        "steady_load_limitations": "Module allowances are engineering scenarios, not guaranteed maxima. PMW3360 RUN4 37mA is typical; module regulator/translator current and operating conditions need verification. Input includes the assumed 1mA host LDO ground reserve.",
        "startup_current_screen": {
            "pmw_vdd_supply_ramp_max_ma": 70,
            "pmw_vddio_supply_ramp_max_ma": 60,
            "conservative_pmw_ramp_sum_ma": 130,
            "host_allocation_ma": 110,
            "host_plus_pmw_ramp_ma": 240,
            "limitations": "Conservative addition of separate PMW supply-ramp limits, not a measured simultaneous current or continuous load. Excludes module regulator/translator overhead, capacitor charging and inrush; not a bound on total USB startup current or a steady thermal scenario."
        },
        "external_module_nominal_capacitance_uf": {
            "connector_input_bulk": 10,
            "connector_input_bypass": 0.1,
            "regulated_downstream_bulk": 20,
            "regulated_downstream_bypass": 0.3,
            "limitations": "Historical schematic nominal values, not measured effective capacitance. Downstream includes the module and sensor regulators; do not add these values directly as USB input capacitance. Regulator startup, voltage conversion, tolerance/DC bias and active loads require an inrush model or measurement."
        }
    }


def usb_geometry(board):
    nets = ("USB_DM", "USB_DP", "USB_DM_MCU", "USB_DP_MCU")
    lengths = {net: defaultdict(float) for net in nets}
    widths = {net: defaultdict(set) for net in nets}
    vias = {net: [] for net in nets}
    ground_vias = [field(x, "at")[:2] for x in board
                   if isinstance(x, list) and str(x[0]) == "via" and field(x, "net") == ["GND"]]
    for x in board:
        if not isinstance(x, list) or not x or str(x[0]) not in ("segment", "via"):
            continue
        net = field(x, "net")[0]
        if net not in nets:
            continue
        if str(x[0]) == "via":
            pos = field(x, "at")[:2]
            vias[net].append({"at_mm": pos, "nearest_ground_via_mm":
                              round(min(math.dist(pos, p) for p in ground_vias), 4) if ground_vias else None})
        else:
            layer = field(x, "layer")[0]
            lengths[net][layer] += math.dist(field(x, "start"), field(x, "end"))
            widths[net][layer].add(field(x, "width")[0])
    if any(not lengths[net] for net in nets):
        raise ValueError("Expected four USB nets with native segments; review changed topology")
    return {net: {"segment_sum_mm": round(sum(lengths[net].values()), 4),
                  "layer_length_mm": {layer: round(value, 4) for layer, value in sorted(lengths[net].items())},
                  "layer_widths_mm": {layer: sorted(values) for layer, values in sorted(widths[net].items())},
                  "vias": sorted(vias[net], key=lambda v: v["at_mm"])} for net in nets}


def main():
    variants = {}
    hashes = {}
    for keys, directory in ((65, ROOT / "design/pcb"), (62, ROOT / "design/pcb/62")):
        circuit_path = directory / "circuit.json"
        circuit = json.loads(circuit_path.read_text())
        parts = {part["ref"]: part for part in circuit["parts"]}
        expected = {"U1": "RP2040", "U2": "W25Q16JVSSIQ", "U3": "AP2112K-3.3TRG1",
                    "F1": "Bourns MF-PSMF050X-2", "DB1": "Unified Daughterboard C5-JSH"}
        for ref, mpn in expected.items():
            if parts[ref]["mpn"] != mpn:
                raise ValueError(f"{keys}: {ref} changed; revisit power assumptions")
        board_path = directory / f"keycult{keys}.kicad_pcb"
        board = sexpdata.loads(board_path.read_text())
        capacitance = {rail: 0.0 for rail in ("VBUS_FUSED", "+3V3", "+1V1")}
        for part in circuit["parts"]:
            if part["lib"] == "Device:C" and part["nets"].get("2") == "GND":
                rail = part["nets"].get("1")
                if rail in capacitance:
                    capacitance[rail] += {"1uF": 1.0, "100nF": 0.1}[part["value"]]
        variants[str(keys)] = {"explicit_stackup": field(field(board, "setup"), "stackup") is not None,
                               "main_board_nominal_rail_capacitance_uf": {rail: round(c, 3) for rail, c in capacitance.items()},
                               "sensor": sensor_review(parts),
                               "usb": usb_geometry(board)}
        for path in (circuit_path, board_path):
            hashes[str(path.relative_to(ROOT))] = hashlib.sha256(path.read_bytes()).hexdigest()
    candidates = []
    for name, height, er, core, proposed_width in (
        ("JLC04161H-7628", 0.2104, 4.4, 1.065, 0.29),
        ("JLC04161H-3313", 0.0994, 4.1, 1.265, 0.16),
    ):
        candidates.append({"name": name, "board_nominal_mm": 1.6,
                           "outer_copper_mm": 0.035, "inner_copper_mm": 0.0152,
                           "outer_to_inner_dielectric_mm": height, "prepreg_er": er,
                           "inner_to_inner_dielectric_mm": core,
                           "assumed_outer_layer_gap_mm": 0.2,
                           "outer_0_2mm_width_estimate_ohms": round(microstrip_ohms(0.2, 0.2, height, 0.035, er), 2),
                           "outer_trial_width_mm": proposed_width,
                           "outer_trial_estimate_ohms": round(microstrip_ohms(proposed_width, 0.2, height, 0.035, er), 2)})
    drops = []
    for extra_ohms in (0.5, 1.0, 2.0):
        total_ohms = 0.9 + 0.13 + extra_ohms
        current_a = 0.151
        ldo_in = 4.4 - current_a * total_ohms
        drops.append({"other_loop_resistance_ohms": extra_ohms,
                      "series_total_ohms": round(total_ohms, 2), "input_current_ma": 151,
                      "device_connector_vbus_v": 4.4, "drop_v": round(current_a * total_ohms, 4),
                      "ldo_input_v": round(ldo_in, 4),
                      "headroom_over_3_5v_screen_v": round(ldo_in - 3.5, 4)})
    result = {
        "status": "conditional desk review; no power, signal-integrity or manufacturing pass",
        "source_facts": "../sources.md#electrical-desk-review-sources",
        "input_sha256": hashes,
        "assumptions": {
            "power": "150mA total 3V3 planning allocation; MCU 80mA and other loads 5mA are engineering allowances, not measured maxima. Flash 25mA requires cited operating conditions and read clock <=104MHz. LDO ground-current reserve 1mA is assumed.",
            "thermal": "VBUS 5.5V, no upstream drop credited; thetaJA 184C/W is datasheet No Heatsink reference, not measured board thermal resistance. 125C is a planning ceiling, not a guaranteed device limit.",
            "capacitance": "Main-board capacitor nominal sums and any separately identified external-module values are not effective capacitance or an inrush model. External daughterboard and cable capacitance, DC bias/tolerance, regulator startup and active load are not modeled; downstream capacitance is not added directly to USB input capacitance.",
            "drop": "4.4V at daughterboard USB receptacle with 151mA is a low-voltage stress scenario, not permission to draw high-power current from a low-power port. Fuse R1max sum 1.03ohm is reference-temperature data, not a hot-resistance bound. Other loop resistance includes internal wire, contacts, PCB supply/ground and vias; external USB cable drop is already represented by receptacle voltage. 3.5V is a nominal3.3V plus0.2V dropout screening threshold, not proof of regulated output across tolerances.",
            "usb": "Segment sums include any branches and exclude pad traversal, via barrels, DB1 and cables. Nearest GND via distance alone does not qualify a return path. Both variants use the 7628 candidate stackup and F.Cu pair over In1.Cu; 62-key DRAFT H has a separate short route. Microstrip estimates omit solder mask, etch, finite plane edges, coplanar copper, tolerances and transitions; they do not qualify the actual routes, local narrow escapes or test-pad branches."
        },
        "power_budget_ma": {"mcu_allocation": 80, "flash_allocation": 25, "other_3v3_allocation": 5,
                            "total_3v3_allocation": 150, "conditional_3v3_sensor_allowance": 40,
                            "ldo_ground_reserve": 1, "input_before_daughterboard_leakage": 151},
        "thermal_scenarios": [thermal(i, t) for t in (25, 50, 60) for i in (100, 150, 200, 300, 500)],
        "thermal_125c_ceiling_at_50c_ma": round(((125 - 50) / 184 - 5.5 * 0.001) / (5.5 - 3.3) * 1000, 2),
        "voltage_drop_scenarios": drops,
        "stackup_candidates": candidates,
        "variants": variants,
        "unresolved": ["Actual sensor hardware revision, loaded logic levels, maximum current and startup under the assumed +3V3 interface", "Firmware current before configuration and during suspend",
                       "Loaded LDO ground current and actual board/case thermal resistance", "Hot PTC and cable/connector loop resistance",
                       "Inrush waveform including capacitance behind regulators", "Fabricator-approved impedance geometry and finite reference-plane/transition effects",
                       "USB cable/connector/ESD/via discontinuities and measured full-speed signal quality"]
    }
    OUTPUT.write_text(json.dumps(result, indent=2) + "\n")
    print(f"Wrote {OUTPUT.relative_to(ROOT)}: conditional estimates only; unresolved items remain")


if __name__ == "__main__":
    main()
