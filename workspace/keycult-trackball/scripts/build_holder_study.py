# /// script
# requires-python = ">=3.11"
# dependencies = ["ezdxf==1.4.3", "sexpdata==1.0.2"]
# ///
"""Test a suspended Keyball support using explicit, unverified holder envelopes."""
import hashlib
import json
import math
from urllib.request import urlopen
import zipfile

import ezdxf
from ezdxf import bbox, disassemble
import sexpdata as sx

from build_plate import App, Part, ROOT, V, feature, rect, sha, source_blank

OUT = ROOT / "design/preliminary/sources/local/holder-study"
UP = ROOT / "design/preliminary/sources/upstream/keyball-holder-review"
INPUTS = {
    "keyball44_Right_KiCad.zip": "d7a7cb8af5bb429e740ddf0d56cb31a22419f0ebe14da25a7fb1b41e09dca694",
    "Keyball44_BottomAcryl_2mm_156x186mm.zip": "391a28ea344900111d1775bd3c570c16fa44d05eb5d253f4f366faad9bad1275",
}
# These values describe test geometry, not a measured production holder or case.
ASSUMPTIONS = dict(ball_radius=17., seat_to_ball_center=20., seat_thickness=2.,
                   seat_z=-12., holder_diameter=44., holder_top_above_center=11.,
                   access_to_ball_x=0., access_to_ball_y=0., pcb_top=-5., pcb_thickness=1.6,
                   connector_board_top_above_seat=4., sensor_height=25., sensor_width=22.,
                   holder_head_diameter=4., holder_head_height=1.5, floor_margin=1.,
                   flange_nut_height=2., flange_screw_tip_below_nut=1.,
                   plate_thickness=1.5, flange_thickness=2., flange_width=52.,
                   window_diameter=46., opening_width=48., opening_rear=-35., opening_front=25.)


def tag(x):
    return str(x[0]) if isinstance(x, list) and x else ""


def child(x, name):
    return next((v for v in x if tag(v) == name), None)


def modules(tree):
    result = {}
    for f in tree:
        if tag(f) not in ("module", "footprint"):
            continue
        ref = next((str(v[2]) for v in f if tag(v) == "property" and v[1] == "Reference"), None)
        if ref is None:
            ref = next(str(v[2]) for v in f if tag(v) == "fp_text" and str(v[1]) == "reference")
        result[ref] = f
    return result


def host_interface():
    UP.mkdir(parents=True, exist_ok=True)
    for name, expected in INPUTS.items():
        if not (UP / name).exists():
            url = 'https://raw.githubusercontent.com/Yowkees/keyball/e1d1948e6e60a83f6b3566a209326508200d64ce/keyball44/design_data/'+name
            data = urlopen(url, timeout=30).read()
            if hashlib.sha256(data).hexdigest() != expected:
                raise ValueError('Downloaded Keyball source hash differs: '+name)
            (UP / name).write_bytes(data)
        if sha(UP / name) != expected:
            raise ValueError("Keyball host source differs: " + name)
    with zipfile.ZipFile(UP / "keyball44_Right_KiCad.zip") as z:
        host = modules(sx.loads(z.read("keyball44_Right_KiCad/keyball44_right.kicad_pcb").decode()))
    with zipfile.ZipFile(UP / "Keyball44_BottomAcryl_2mm_156x186mm.zip") as z:
        import io
        dxf = ezdxf.read(io.StringIO(z.read("KB44_Bottom_2mm_156x186.dxf").decode(), newline=None))
    boxes = [bbox.extents([e]) for e in disassemble.recursive_decompose(dxf.modelspace())]
    # Entity order is fixed by the verified archive hash; seven independent PCB datums set scale.
    pairs = [(child(host[ref], "at")[1:3], boxes[i].center) for i, ref in
             {0: "J11", 43: "J9", 44: "J5", 45: "J10", 46: "J8", 47: "J6", 48: "J7"}.items()]
    px, py, dx, dy = [sum(v) / len(v) for v in zip(*[(p[0], -p[1], d.x, d.y) for p, d in pairs])]
    scale = sum((p[0]-px)*(d.x-dx)+(-p[1]-py)*(d.y-dy) for p, d in pairs) / sum((p[0]-px)**2+(-p[1]-py)**2 for p, d in pairs)
    tx, ty = dx-scale*px, dy-scale*py
    residual = max(math.hypot(scale*p[0]+tx-d.x, -scale*p[1]+ty-d.y) for p, d in pairs)
    if residual > .001 or not .999 < scale < 1.001:
        raise ValueError("Host registration no longer matches the seven PCB mounting datums")
    access = boxes[51].center
    holes = [((access.y-b.center.y)/scale, -(b.center.x-access.x)/scale) for b in (boxes[49], boxes[50])]
    connector = child(host["J2"], "at")[1:3]
    connector_xy = ((connector[1]-(ty-access.y)/scale), -(connector[0]-(access.x-tx)/scale))
    return dict(holes_mm=holes, hole_diameter_mm=2.5, access_diameter_mm=12., connector_xy_mm=connector_xy,
                raw_dxf_insunits=dxf.units, registration_scale=scale, registration_max_residual=residual,
                datum="Access-hole center; its relation to ball center is ASSUMED, not measured",
                orientation="Host +X becomes layout -Y; host +Y becomes layout +X", archive_sha256=INPUTS)


def pcb_inputs(selector, keys):
    path = ROOT / "design/pcb" / ("62" if selector == 62 else "") / f"keycult{selector}.kicad_pcb"
    data = path.read_bytes()
    tree = sx.loads(data.decode())
    stems = []
    for ref, f in modules(tree).items():
        if not ref.startswith("SW") or 'SW_Cherry_MX_' not in str(f[1]):
            continue
        at = child(f, "at")[1:]
        angle = math.radians(at[2] if len(at) > 2 else 0.)
        pads = [p for p in f if tag(p) == "pad" and p[1] == "" and child(p, "drill") and child(p, "drill")[1] > 3.9]
        if len(pads) != 1:
            raise ValueError("Expected one switch stem: " + ref)
        x, y = child(pads[0], "at")[1:3]
        stems.append((at[0]+x*math.cos(angle)+y*math.sin(angle)-180,
                      at[1]-x*math.sin(angle)+y*math.cos(angle)-100))
    errors = [min(math.dist((k['x_mm'], k['y_mm']), p) for p in stems) for k in keys]
    if len(stems) != len(keys) or max(errors) > 1e-5 or len(set(stems)) != len(stems):
        raise ValueError("Native PCB stems and layout do not agree")
    edges = [e for e in tree if tag(e).startswith("gr_") and child(e, "layer") and child(e, "layer")[1] == "Edge.Cuts"]
    if len(edges) != 4 or any(tag(e) != "gr_line" for e in edges):
        raise ValueError("Revisit board-envelope model: native Edge.Cuts is no longer four straight edges")
    pts = [tuple(child(e, "start")[1:3]) for e in edges] + [tuple(child(e, "end")[1:3]) for e in edges]
    xs, ys = sorted(set(p[0] for p in pts)), sorted(set(p[1] for p in pts))
    if len(xs) != 2 or len(ys) != 2 or len(set(pts)) != 4:
        raise ValueError("Native board envelope is not rectangular")
    return path, hashlib.sha256(data).hexdigest(), (xs[0]-180, ys[0]-100, xs[1]-180, ys[1]-100), max(errors)


def box(x, y, z, w, d, h):
    return Part.makeBox(w, d, h, V(x, y, z))


def valid(shape, name, single=True):
    if not shape.isValid() or (single and len(shape.Solids) != 1):
        raise ValueError("Invalid geometry: " + name)


def geometry(interface, seat_z, diameter):
    s = ASSUMPTIONS
    ball_z = seat_z+s['seat_to_ball_center']
    flange_bottom = -s['plate_thickness']-s['flange_thickness']
    window = Part.makeCylinder(s['window_diameter']/2, 80, V(0, 0, -30)).fuse(box(-14, -36, -30, 28, 36, 80))
    flange = box(-s['flange_width']/2, -40, flange_bottom, s['flange_width'], 66, s['flange_thickness']).cut(window)
    mount_xy = [(x, y) for x in (-21, 21) for y in (-21, 21)]
    flange = flange.cut(Part.makeCompound([Part.makeCylinder(1.1, s['flange_thickness'], V(x, y, flange_bottom)) for x, y in mount_xy]))
    seat = box(-24, -34, seat_z-s['seat_thickness'], 48, 58, s['seat_thickness'])
    ox, oy = s['access_to_ball_x'], s['access_to_ball_y']
    holes = [(x+ox, y+oy) for x, y in interface['holes_mm']]
    seat = seat.cut(Part.makeCylinder(interface['access_diameter_mm']/2, s['seat_thickness'], V(ox, oy, seat_z-s['seat_thickness'])))
    seat = seat.cut(Part.makeCompound([Part.makeCylinder(interface['hole_diameter_mm']/2, s['seat_thickness'], V(x, y, seat_z-s['seat_thickness'])) for x, y in holes]))
    posts = Part.makeCompound([box(x-2, y-2, seat_z, 4, 4, flange_bottom-seat_z) for x, y in mount_xy])
    cx, cy = interface['connector_xy_mm']
    cx, cy = cx+ox, cy+oy
    board_top = seat_z+s['connector_board_top_above_seat']
    support = Part.makeCompound([box(cx+x-2, cy-5, seat_z, 4, 10, board_top-1.6-seat_z) for x in (-16, 16)])
    connector_board = box(cx-18, cy-5, board_top-1.6, 36, 10, 1.6)
    board_screws = Part.makeCompound([Part.makeCylinder(1.1, s['connector_board_top_above_seat']+s['seat_thickness'], V(cx+x, cy, seat_z-s['seat_thickness'])) for x in (-16, 16)])
    support = support.cut(board_screws)
    seat = seat.cut(board_screws)
    connector_board = connector_board.cut(board_screws)
    connector_heads = Part.makeCompound([Part.makeCylinder(2, 2, V(cx+x, cy, board_top)) for x in (-16, 16)])
    sensor = box(cx-s['sensor_width']/2, cy-.8, board_top, s['sensor_width'], 1.6, s['sensor_height'])
    mount_bottom = seat_z-s['seat_thickness']-s['flange_nut_height']-s['flange_screw_tip_below_nut']
    mount_tools = Part.makeCompound([Part.makeCylinder(1.1, -mount_bottom, V(x, y, mount_bottom)) for x, y in mount_xy])
    flange, seat, posts = (part.cut(mount_tools) for part in (flange, seat, posts))
    basket = flange.fuse(seat).fuse(posts).fuse(support).removeSplitter()
    fasteners = []
    for x, y in mount_xy:
        nut_z = seat_z-s['seat_thickness']-s['flange_nut_height']
        vertices = [V(x+2.5*math.cos(i*math.pi/3), y+2.5*math.sin(i*math.pi/3), nut_z) for i in range(7)]
        nut = Part.Face(Part.makePolygon(vertices)).extrude(V(0, 0, s['flange_nut_height']))
        shaft = Part.makeCylinder(1, -mount_bottom, V(x, y, mount_bottom))
        fasteners.append(shaft.fuse(nut).fuse(Part.makeCylinder(2.1, 2, V(x, y, 0))))
    flange_fasteners = Part.makeCompound(fasteners)
    valid(basket, "suspended basket")
    head_bottom = seat_z-s['seat_thickness']-s['holder_head_height']
    heads = Part.makeCompound([Part.makeCylinder(s['holder_head_diameter']/2, s['holder_head_height'], V(x, y, head_bottom)) for x, y in holes])
    drivers = Part.makeCompound([Part.makeCylinder(2.5, 10, V(x, y, head_bottom-10)) for x, y in holes])
    ball = Part.makeSphere(s['ball_radius'], V(0, 0, ball_z))
    holder = holder_envelope(interface, seat_z, diameter)
    opening = box(-s['opening_width']/2, s['opening_rear'], -30, s['opening_width'], s['opening_front']-s['opening_rear'], 80)
    return dict(Window=window, Flange=flange, Seat=seat, Posts=posts, ConnectorSupport=support,
                Basket=basket, ConnectorBoardProxy=connector_board, ConnectorScrewHeadsProxy=connector_heads, SensorBoardProxy=sensor,
                HolderScrewHeadsProxy=heads, FlangeFastenersProxy=flange_fasteners, DriverAccessProxy=drivers, Ball=ball,
                HolderEnvelopeAssumption=holder, ProposedPCBOpeningTool=opening)


def holder_envelope(interface, seat_z, diameter):
    # shortcut: generic occupied volume; replace with the acquired holder CAD before fit approval.
    s = ASSUMPTIONS
    height = s['seat_to_ball_center']+s['holder_top_above_center']
    cx = interface['connector_xy_mm'][0]+s['access_to_ball_x']
    return Part.makeCylinder(diameter/2, height, V(0, 0, seat_z)).fuse(box(cx-13, -33+s['access_to_ball_y'], seat_z, 26, 17, height))


def section_svg(folder, shapes, seat_z):
    plane = Part.Face(Part.makePolygon([V(0, -50, -30), V(0, 40, -30), V(0, 40, 40), V(0, -50, 40), V(0, -50, -30)]))
    paths = []
    for name, color in [('CurrentPCB', '#ef4444'), ('Plate', '#475569'), ('Basket', '#0f766e'),
                        ('HolderEnvelopeAssumption', '#2563eb'), ('Ball', '#ea580c'), ('SensorBoardProxy', '#7e22ce')]:
        for edge in shapes[name].section(plane).Edges:
            points = edge.discretize(Deflection=.05)
            d = 'M'+' L'.join(f'{p.y:.4f},{-p.z:.4f}' for p in points)
            dash = 'stroke-dasharray="1 .7"' if name == 'HolderEnvelopeAssumption' else ''
            paths.append(f'<path d="{d}" fill="none" stroke="{color}" stroke-width=".35" {dash}/>')
    floor = -min(shapes['HolderScrewHeadsProxy'].BoundBox.ZMin, shapes['FlangeFastenersProxy'].BoundBox.ZMin)+ASSUMPTIONS['floor_margin']
    (folder / 'section.svg').write_text(f'''<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="900" viewBox="-52 -45 100 90">
<rect x="-52" y="-45" width="100" height="90" fill="white"/>
<g font-family="sans-serif" font-size="2.3" fill="#172033">
<text x="-48" y="-39">CONDITIONAL STUDY / X=0 SECTION / mm / NOT FOR FABRICATION</text>
<text x="-48" y="-35">Blue dashed: unknown holder envelope assumption, not original CAD</text>
<text x="-48" y="-31">Teal: bracket / Orange: ball / Purple: sensor board proxy</text>
{''.join(paths)}
<path d="M-48,0 L43,0" stroke="#94a3b8" stroke-width=".15" stroke-dasharray="1 1"/>
<text x="28" y="-1">Plate Z=0</text>
<path d="M-48,{floor} L43,{floor}" stroke="#a21caf" stroke-width=".2" stroke-dasharray="1 1"/>
<text x="-48" y="{floor+4}">Required floor below Z=-{floor:g} for assumed seat/fasteners + 1mm margin</text>
<text x="-48" y="{floor+8}">Actual case floor and remaining hardware are NOT modeled</text>
<text x="-48" y="37">Red: CURRENT uncut PCB. Comparison model uses a 48 x 60mm opening.</text>
<text x="-48" y="41">Left = rear (-Y). Holder seat Z={seat_z:g}; ball-center/seat offset assumed 20mm.</text>
</g></svg>''')


def run_variant(selector, keys, face, blank, interface):
    path, digest, bounds, stem_error = pcb_inputs(selector, keys)
    key_count = len(keys)
    near = [k for k in keys if abs(k['x_mm']) < 50 and abs(k['y_mm']) < 60]
    cap_faces = [rect(k['x_mm']-(18+(k.get('width_u', 1)-1)*19.05)/2, k['y_mm']-9,
                      18+(k.get('width_u', 1)-1)*19.05, 18, 5) for k in near]
    caps = Part.makeCompound([f.extrude(V(0, 0, 10)) for f in cap_faces])
    switches = Part.makeCompound([box(k['x_mm']-7, k['y_mm']-7, -9, 14, 14, 14) for k in near])
    switch_cuts = Part.makeCompound([box(k['x_mm']-7, k['y_mm']-7, -1.5, 14, 14, 1.5) for k in keys])
    x0, y0, x1, y1 = bounds
    s = ASSUMPTIONS
    board = box(x0, y0, s['pcb_top']-s['pcb_thickness'], x1-x0, y1-y0, s['pcb_thickness'])
    cases = []
    for seat_z in (-8., -12., -16.):
        g = geometry(interface, seat_z, 44.)
        for diameter in (40., 44., 48.):
            envelope = holder_envelope(interface, seat_z, diameter)
            opened = board.cut(g['ProposedPCBOpeningTool'])
            plate = blank.cut(switch_cuts).cut(g['Window'])
            mount_holes = Part.makeCompound([Part.makeCylinder(1.1, 1.5, V(x, y, -1.5)) for x in (-21, 21) for y in (-21, 21)])
            plate = plate.cut(mount_holes).removeSplitter()
            parts = ['Seat', 'Posts', 'Flange', 'ConnectorSupport', 'ConnectorBoardProxy', 'SensorBoardProxy', 'Ball', 'HolderScrewHeadsProxy', 'ConnectorScrewHeadsProxy', 'FlangeFastenersProxy']
            clash = {name: g[name].common(board).Volume for name in parts}
            open_clash = {name: g[name].common(opened).Volume for name in parts}
            flange_face = max((f for f in g['Flange'].Faces if abs(f.CenterOfMass.z+s['plate_thickness']) < 1e-6 and f.normalAt(0, 0).z > .99), key=lambda f: f.Area)
            plate_face = max((f for f in plate.Faces if abs(f.CenterOfMass.z+s['plate_thickness']) < 1e-6 and f.normalAt(0, 0).z < -.99), key=lambda f: f.Area)
            interface_face = flange_face.Area
            support_area = flange_face.common(plate_face).Area
            restored = Part.Face(plate_face.OuterWire)
            restored.translate(V(0, 0, s['plate_thickness']))
            perimeter_error = restored.cut(face).Area+face.cut(restored).Area
            if perimeter_error > 1e-5:
                raise ValueError('Experimental window changed the official outer boundary')
            sensor_clash = {name: dict(keycaps_mm3=g[name].common(caps).Volume, switches_mm3=g[name].common(switches).Volume) for name in ('SensorBoardProxy', 'ConnectorBoardProxy')}
            row = dict(seat_z_mm=seat_z, holder_diameter_assumed_mm=diameter,
                       ball_tip_z_mm=seat_z+s['seat_to_ball_center']+s['ball_radius'],
                       required_floor_depth_with_assumed_fasteners_and_1mm_gap_mm=-min(g['HolderScrewHeadsProxy'].BoundBox.ZMin, g['FlangeFastenersProxy'].BoundBox.ZMin)+s['floor_margin'],
                       current_pcb_intersections_mm3=clash, proposed_open_pcb_intersections_mm3=open_clash,
                       holder_envelope_vs_open_pcb_mm3=envelope.common(opened).Volume,
                       holder_envelope_vs_plate_mm3=envelope.common(plate).Volume,
                       holder_envelope_vs_basket_mm3=envelope.common(g['Basket']).Volume,
                       basket_vs_switch_proxies_mm3=g['Basket'].common(switches).Volume,
                       holder_envelope_vs_keycaps_mm3=envelope.common(caps).Volume,
                       holder_envelope_vs_switches_mm3=envelope.common(switches).Volume,
                       sensor_proxy_intersections=sensor_clash,
                       ball_vs_basket_mm3=g['Ball'].common(g['Basket']).Volume,
                       basket_vs_plate_mm3=g['Basket'].common(plate).Volume,
                       connector_screw_heads_vs_basket_mm3=g['ConnectorScrewHeadsProxy'].common(g['Basket']).Volume,
                       flange_fasteners_vs_basket_mm3=g['FlangeFastenersProxy'].common(g['Basket']).Volume,
                       flange_fasteners_vs_plate_mm3=g['FlangeFastenersProxy'].common(plate).Volume,
                       outer_boundary_area_error_mm2=perimeter_error,
                       driver_vs_basket_mm3=g['DriverAccessProxy'].common(g['Basket']).Volume,
                       flange_top_area_mm2=interface_face, flange_plate_contact_area_mm2=support_area,
                       window_vs_switch_openings_mm3=g['Window'].common(switch_cuts).Volume)
            row['conditional_proxy_checks_pass'] = all(v < 1e-6 for v in open_clash.values()) and all(v < 1e-6 for pair in sensor_clash.values() for v in pair.values()) and all(row[n] < 1e-6 for n in (
                'holder_envelope_vs_open_pcb_mm3', 'holder_envelope_vs_plate_mm3', 'holder_envelope_vs_basket_mm3',
                'basket_vs_switch_proxies_mm3', 'holder_envelope_vs_keycaps_mm3', 'holder_envelope_vs_switches_mm3',
                'ball_vs_basket_mm3', 'basket_vs_plate_mm3', 'connector_screw_heads_vs_basket_mm3', 'flange_fasteners_vs_basket_mm3', 'flange_fasteners_vs_plate_mm3', 'driver_vs_basket_mm3', 'window_vs_switch_openings_mm3')) and interface_face > 0 and abs(support_area-interface_face) < 1e-5
            cases.append(row)
            if seat_z == s['seat_z'] and diameter == s['holder_diameter']:
                folder = OUT / str(selector)
                folder.mkdir(parents=True, exist_ok=True)
                doc = App.newDocument(f'HolderStudy{selector}')
                info = doc.addObject('App::FeaturePython', 'Assumptions')
                info.addProperty('App::PropertyString', 'Warning').Warning = 'NOT ACTUAL KEYBALL HOLDER CAD. Conditional envelopes only; no fit/optics/strength approval.'
                info.addProperty('App::PropertyString', 'ParametersJSON').ParametersJSON = json.dumps(ASSUMPTIONS)
                info.addProperty('App::PropertyString', 'EditInstructions').EditInstructions = 'Edit scripts/build_holder_study.py and regenerate; displayed properties are records only.'
                for name in ('Basket', 'Ball', 'ConnectorBoardProxy', 'SensorBoardProxy', 'HolderScrewHeadsProxy', 'ConnectorScrewHeadsProxy', 'FlangeFastenersProxy', 'DriverAccessProxy', 'HolderEnvelopeAssumption'):
                    feature(doc, name, g[name], 'ASSUMED test geometry; actual holder and sensor mating geometry unavailable')
                for name, shape, note in [('Plate', plate, 'Official outer contour; experimental window and fastener cuts'),
                    ('CurrentPCB', board, 'Native XY boundary; Z=-5 and 1.6mm thickness assumed; uncut'),
                    ('ProposedOpenPCB', opened, 'Comparison ONLY; opening is not applied in KiCad'),
                    ('KeycapProxies', caps, 'Current XY widths; Z=5..15 assumed'),
                    ('SwitchBodyProxies', switches, '14mm squares at Z=-9..5; no actual clips/sockets model'),
                    ('CurrentPCBClash', board.common(g['Basket'].fuse(g['Ball']).fuse(envelope)), 'Known collision with current uncut PCB')]:
                    feature(doc, name, shape, note)
                for name, shape in [('Basket', g['Basket']), ('Plate', plate), ('ProposedOpenPCB', opened)]:
                    valid(shape, name)
                    Part.export([doc.getObject(name)], str(folder / f'{name}-EXPERIMENT.step'))
                    readback = Part.read(str(folder / f'{name}-EXPERIMENT.step'))
                    valid(readback, name+' STEP readback')
                    if abs(shape.Volume-readback.Volume) > 1e-4:
                        raise ValueError('STEP volume mismatch: '+name)
                doc.recompute()
                doc.saveAs(str(folder / 'holder-study.FCStd'))
                App.closeDocument(doc.Name)
                reopened = App.openDocument(str(folder / 'holder-study.FCStd'))
                valid(reopened.getObject('Basket').Shape, 'FCStd readback')
                App.closeDocument(reopened.Name)
                section_svg(folder, {**g, 'Plate': plate, 'CurrentPCB': board}, seat_z)
        print(f'Completed {key_count} keys, seat Z {seat_z}', flush=True)
    if sha(path) != digest:
        raise ValueError('PCB changed during experiment; rerun with stable inputs')
    return dict(selector=selector, key_count=key_count, pcb_sha256=digest, native_stem_error_mm=stem_error, cases=cases)


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    layout_path = ROOT / 'design/preliminary/layout.json'
    layout_digest = sha(layout_path)
    layout = json.loads(layout_path.read_text())
    interface = host_interface()
    face, blank, _ = source_blank()
    variants = [run_variant(n, layout[key]['keys'], face, blank, interface) for n, key in ((65, 'preferred'), (62, 'rear_clearance'))]
    if sha(layout_path) != layout_digest:
        raise ValueError('Layout changed during experiment; rerun with stable inputs')
    report = dict(status='Conditional CAD experiment, not manufacturing or actual-holder fit approval',
                  assumptions=ASSUMPTIONS, host_interface=interface, layout_sha256=layout_digest, variants=variants,
                  unresolved=['Actual holder envelope and access-hole to ball-center datum', 'Seat/connector/sensor optical stack',
                              'Case floor, ribs and opening', 'Fastener engagement, bracket material and flex',
                              'Holder insertion/removal and driver access in the assembled case', 'Real switch, keycap travel and component envelopes'])
    (OUT / 'review.json').write_text(json.dumps(report, indent=2)+'\n')
    print('Saved', OUT, flush=True)


if __name__ == '__main__':
    main()
