# /// script
# requires-python = ">=3.11"
# dependencies = ["sexpdata==1.0.2"]
# ///
"""Deterministic USB overlay on the saved matrix route; candidate 7628 stackup."""
from pathlib import Path
import uuid

import sexpdata as sx

USB_NETS = {"USB_DM", "USB_DP"}
WIDTH = 0.29


def node(name, *args):
    return [sx.Symbol(name), *args]


def child(tree, name):
    return next((x for x in tree if isinstance(x, list) and x and str(x[0]) == name), None)


def apply_usb_route(path):
    """Replace only connector-side USB tracks/vias and this module's GND vias."""
    path = Path(path)
    board = sx.loads(path.read_text())
    def uid(label):
        return str(uuid.uuid5(uuid.NAMESPACE_URL, "keycult-ball/usb-route-e/" + label))
    ground_points = [(38.5, 113.2), (113.4, 55.7), (118.5, 54.7)]
    ground_ids = {uid("gnd/" + str(p)) for p in ground_points} | {uid("zone" + str(i)) for i in range(3)}
    clean = []
    for x in board:
        if isinstance(x, list) and x and str(x[0]) in ("segment", "via", "zone"):
            net = child(x, "net")
            identifier = child(x, "uuid")
            # SES import replaces UUIDs, so recognize our stitching vias by geometry.
            at = child(x, "at")
            imported_stitch = (str(x[0]) == "via" and net == node("net", "GND")
                               and at and tuple(at[1:]) in ground_points
                               and child(x, "size") == node("size", .6)
                               and child(x, "drill") == node("drill", .3)
                               and child(x, "layers") == node("layers", "F.Cu", "B.Cu"))
            if imported_stitch:
                continue
            if (str(x[0]) in ("segment", "via") and net and net[1] in USB_NETS) or (identifier and identifier[1] in ground_ids):
                continue
        clean.append(x)
    board = clean
    def route(net, points, layer, label):
        for i, (a, b) in enumerate(zip(points, points[1:])):
            if a == b:
                continue
            board.append(node("segment", node("start", *a), node("end", *b),
                              node("width", WIDTH), node("layer", layer), node("net", net),
                              node("uuid", uid(label + str(i)))))
    def via(net, p, label):
        board.append(node("via", node("at", *p), node("size", 0.6), node("drill", 0.3),
                          node("layers", "F.Cu", "B.Cu"), node("net", net), node("uuid", uid(label))))
    # The two connector vias flare apart so a .6mm pad clears the opposite trace.
    route("USB_DM", [(42.6,112.5),(39.49,112.5)], "B.Cu", "dm-j1")
    route("USB_DP", [(42.6,111.5),(38.8,111.5)], "B.Cu", "dp-j1")
    via("USB_DM", (39.49,112.5), "dm-source")
    via("USB_DP", (38.8,111.5), "dp-source")
    # Offset 45-degree bends preserve the .2mm edge gap on the long pair.
    route("USB_DM", [(39.49,112.5),(39.49,54.29),(39.79,53.99),(113.9,53.99),(113.9,54.5)], "F.Cu", "dm-margin")
    route("USB_DP", [(38.8,111.5),(38.8,110.8),(39,110.6),(39,54.087035),(39.587035,53.5),(117.1,53.5),(117.1,54.5)], "F.Cu", "dp-margin")
    for net, points, label in [
        ("USB_DM", [(113.9,54.5),(114.8,54.5)], "dm-test"),
        ("USB_DP", [(117.1,54.5),(116.4,55.5)], "dp-test"),
        ("USB_DM", [(115.4,67.5),(114.51,67.5)], "dm-r1"),
        ("USB_DP", [(116.4,68.7),(114.51,68.7)], "dp-r2"),
    ]:
        route(net, points, "B.Cu", label)
    for net, points, label in [
        ("USB_DM", [(114.8,54.5),(114.8,56.1),(115.4,56.7),(115.4,67.5)], "dm-local"),
        ("USB_DP", [(116.4,55.5),(116.4,56.392965),(115.89,56.902965),(115.89,66.19),(116.4,66.7),(116.4,68.7)], "dp-local"),
    ]:
        route(net, points, "F.Cu", label)
    for net, points in [("USB_DM", [(113.9,54.5),(114.8,54.5),(115.4,67.5)]),
                        ("USB_DP", [(117.1,54.5),(116.4,55.5),(116.4,68.7)])]:
        for p in points:
            via(net, p, net + str(p))
    for p in ground_points:
        via("GND", p, "gnd/" + str(p))
    # Extend the top reference plane inside the unchanged .25mm edge rule.
    for x in board:
        if isinstance(x, list) and x and str(x[0]) == "zone" and child(x, "net") == node("net", "GND") and child(x, "layer") == node("layer", "In1.Cu"):
            polygon = child(x, "polygon")
            if polygon:
                for p in child(polygon, "pts")[1:]:
                    if abs(p[2] - 53.2) < 0.00001:
                        p[2] = 52.96
    # These B.Cu reference patches connect the new stitching vias to In1.Cu.
    for i, (left, top, right, bottom) in enumerate([(38.05,106.8,45.8,114.8), (112.4,53.2,119,56.4), (113.8,66.2,118.5,70.3)]):
        board.append(node("zone", node("net", "GND"), node("layer", "B.Cu"), node("uuid", uid("zone" + str(i))),
                          node("hatch", sx.Symbol("edge"), .5), node("connect_pads", sx.Symbol("yes"), node("clearance", .2)),
                          node("min_thickness", .15), node("fill", sx.Symbol("yes"), node("thermal_gap", .5), node("thermal_bridge_width", .5)),
                          node("polygon", node("pts", *[node("xy", *p) for p in [(left,top),(right,top),(right,bottom),(left,bottom)]]))))
    setup = child(board, "setup")
    old = child(setup, "stackup")
    if old:
        setup.remove(old)
    layers = []
    for name, kind, thick, er, material in [
        ("F.Cu", "copper", .035, None, None),
        ("dielectric 1", "prepreg", .2104, 4.4, "7628"),
        ("In1.Cu", "copper", .0152, None, None),
        ("dielectric 2", "core", 1.065, 4.6, "FR4"),
        ("In2.Cu", "copper", .0152, None, None),
        ("dielectric 3", "prepreg", .2104, 4.4, "7628"),
        ("B.Cu", "copper", .035, None, None),
    ]:
        layer = node("layer", name, node("type", kind), node("thickness", thick))
        if er:
            layer += [node("material", material), node("epsilon_r", er)]
        layers.append(layer)
    setup.append(node("stackup", *layers))
    path.write_text(sx.dumps(board) + "\n")


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("board", type=Path)
    apply_usb_route(parser.parse_args().board)
