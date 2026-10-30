# /// script
# requires-python = ">=3.11"
# ///
"""Build an independent printable holder prototype using installed FreeCAD."""
import hashlib
import itertools
import json
import math
import os
from pathlib import Path
import sys
import zipfile

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "design/printable-holder"
sys.path.insert(0, str(Path(os.environ.get("FREECAD_ROOT", "/Applications/FreeCAD.app/Contents/Resources")) / "lib"))
try:
    import FreeCAD as App
    import Part
    import Mesh
    import MeshPart
except ImportError as error:
    raise SystemExit("Use uv run with the installed FreeCAD Python; set FREECAD_ROOT to its resource directory") from error

V = App.Vector
# Prototype choices, not measured dimensions of the purchased sensor or case.
S = dict(ball_radius=17., ball_z=8., bearing_radius=1., cavity_clearance=.5,
         cup_radius=22., bottom_z=-12., split_z=8., ring_top_z=16.,
         pcb_top_z=-5., pcb_thickness=1.6, plate_thickness=1.5,
         sensor_width=22., sensor_height=25., sensor_thickness=1.6,
         sensor_y=-26.5, sensor_bottom_z=-4.5, sensor_travel=3.,
         deck_bottom_z=-10., deck_top_z=-7., mount_pitch=42.,
         ring_screw_radius=19.8, pilot_diameter=1.7, clearance_diameter=2.3)


def box(x, y, z, w, d, h):
    return Part.makeBox(w, d, h, V(x, y, z))


def cylinder(radius, z, height, x=0., y=0.):
    return Part.makeCylinder(radius, height, V(x, y, z))


def valid(shape, name):
    if not shape.isValid() or len(shape.Solids) != 1 or shape.Volume <= 0:
        raise ValueError("Expected one valid solid: " + name)


def rounded_box(bounds, radius, z, height):
    x0, y0, x1, y1 = bounds
    w, d = x1-x0, y1-y0
    shape = box(x0+radius, y0, z, w-2*radius, d, height).fuse(box(x0, y0+radius, z, w, d-2*radius, height))
    for x in (x0+radius, x1-radius):
        for y in (y0+radius, y1-radius):
            shape = shape.fuse(cylinder(radius, z, height, x, y))
    return shape.removeSplitter()


def geometry():
    s = S
    assert s['split_z'] == s['ball_z'] and s['bottom_z'] < s['ball_z']-s['ball_radius']-1
    assert 0 < s['cavity_clearance'] < s['bearing_radius']
    assert s['sensor_travel'] == 3., 'Slot geometry must change with travel'
    center = V(0, 0, s['ball_z'])
    ball = Part.makeSphere(s['ball_radius'], center)
    cavity = Part.makeSphere(s['ball_radius']+s['cavity_clearance'], center)
    base = cylinder(s['cup_radius'], s['bottom_z'], s['split_z']-s['bottom_z']).cut(cavity)
    ring = cylinder(s['cup_radius'], s['split_z'], s['ring_top_z']-s['split_z']).cut(cavity)
    # A cylindrical extension opens the spherical cavity through the top face.
    ring = ring.cut(cylinder(math.sqrt((s['ball_radius']+s['cavity_clearance'])**2-(s['ring_top_z']-s['ball_z'])**2),
                             s['ring_top_z']-0.01, 1.))
    optical_window = Part.makeCylinder(5.5, 15, V(0, -30, s['ball_z']), V(0, 1, 0))
    base, ring = (p.cut(optical_window) for p in (base, ring))
    # Three fixed ceramic balls are inserted from inside the cup and bonded below contact.
    bearings = []
    for angle in (90, 210, 330):
        a = math.radians(angle)
        radial = (s['ball_radius']+s['bearing_radius'])/math.sqrt(2)
        p = V(radial*math.cos(a), radial*math.sin(a), s['ball_z']-radial)
        bearing = Part.makeSphere(s['bearing_radius'], p)
        bore = Part.makeCylinder(s['bearing_radius']+.1, 4., p, (center-p).normalize())
        base = base.cut(bearing.fuse(bore))
        bearings.append(bearing)
        assert abs((p-center).Length-s['ball_radius']-s['bearing_radius']) < 1e-8
    # Ears contact the underside of a separately revised aluminium plate.
    for x in (-s['mount_pitch']/2, s['mount_pitch']/2):
        for y in (-s['mount_pitch']/2, s['mount_pitch']/2):
            ear = box(min(x, 0)-3, min(y, 0)-3, -3.5, abs(x)+6, abs(y)+6, 2).cut(cavity)
            base = base.fuse(ear)
    for x in (-s['mount_pitch']/2, s['mount_pitch']/2):
        for y in (-s['mount_pitch']/2, s['mount_pitch']/2):
            base = base.cut(cylinder(s['clearance_diameter']/2, -4, 3, x, y))
    deck = box(-19, -34, s['deck_bottom_z'], 38, 20, s['deck_top_z']-s['deck_bottom_z'])
    base = base.fuse(deck).cut(cavity)
    base = base.cut(box(-15, -34, -5, 30, 14, 14))
    for x in (-16., 16.):
        slot = cylinder(1.15, -11, 5, x, -30.5).fuse(cylinder(1.15, -11, 5, x, -24.5)).fuse(box(x-1.15, -30.5, -11, 2.3, 6, 5))
        base = base.cut(slot)
    for angle in (45, 135, 225, 315):
        a = math.radians(angle)
        x, y = s['ring_screw_radius']*math.cos(a), s['ring_screw_radius']*math.sin(a)
        base = base.cut(cylinder(s['pilot_diameter']/2, 2, 7, x, y))
        ring = ring.cut(cylinder(s['clearance_diameter']/2, 7, 10, x, y))

    y = s['sensor_y']
    z0 = s['sensor_bottom_z']
    top = z0+s['sensor_height']+.2
    carriage = box(-19, y-4, -7, 38, 4, 2)
    # Open middle clears components; provisional edge channels require real-board checking.
    for x in (-14., 10.):
        carriage = carriage.fuse(box(x, y-2.5, -5, 4, 5, top+5))
    channel = box(-s['sensor_width']/2-.2, y-s['sensor_thickness']/2-.15, z0,
                  s['sensor_width']+.4, s['sensor_thickness']+.3, 30)
    carriage = carriage.cut(channel)
    cap = box(-14, y-2.5, top, 28, 5, 2)
    for x in (-12.5, 12.5):
        carriage = carriage.cut(cylinder(s['pilot_diameter']/2, top-6, 7, x, y))
        cap = cap.cut(cylinder(s['clearance_diameter']/2, top-.1, 3, x, y))
    for x in (-16., 16.):
        carriage = carriage.cut(cylinder(s['clearance_diameter']/2, -8, 4, x, y-1))
    sensor = box(-s['sensor_width']/2, y-s['sensor_thickness']/2, z0,
                 s['sensor_width'], s['sensor_thickness'], s['sensor_height'])
    # Compare seat fits before bonding the actual ceramic contacts.
    coupon = box(0, 0, 0, 30, 12, 4)
    for x, radius in ((5., 1.), (15., 1.05), (25., 1.1)):
        p = V(x, 6, 3)
        coupon = coupon.cut(Part.makeSphere(radius, p).fuse(Part.makeCylinder(radius+.1, 3, p, V(0, 0, 1))))
    coupon = coupon.cut(cylinder(s['pilot_diameter']/2, -1, 6, 10, 9))
    coupon = coupon.cut(cylinder(s['clearance_diameter']/2, -1, 6, 20, 9))
    return {"Cup": base.removeSplitter(), "RetainingRing": ring.removeSplitter(),
            "SensorCarriage": carriage.removeSplitter(), "SensorCap": cap.removeSplitter(),
            "SeatCoupon": coupon.removeSplitter()}, {"Ball": ball, "SensorBoardAssumption": sensor,
                                                       **{f"CeramicBall{i+1}": b for i, b in enumerate(bearings)}}


def check(parts, references):
    layout_path = ROOT / 'design/preliminary/layout.json'
    pcb_path = ROOT / 'design/pcb/62/keycult62.kicad_pcb'
    report = json.loads((pcb_path.parent / 'reports/validation.json').read_text())
    digest = hashlib.sha256(pcb_path.read_bytes()).hexdigest()
    layout = json.loads(layout_path.read_text())['rear_clearance']
    assert report['artifact_sha256'][pcb_path.name] == digest, 'Revalidate modified native PCB first'
    assert report['pcb_opening'] == layout['pcb_opening'], 'Layout/PCB opening mismatch'
    bounds = report['envelope_bounds_mm']
    pcb = box(bounds[0]-180, bounds[1]-100, S['pcb_top_z']-S['pcb_thickness'],
              bounds[2]-bounds[0], bounds[3]-bounds[1], S['pcb_thickness'])
    hole = report['pcb_opening']
    pcb = pcb.cut(rounded_box(hole['bounds_mm'], hole['corner_radius_mm'], -8, 5))
    # Generic local plate coupon only; no upstream outline is reproduced.
    plate = box(-26, -37, -1.5, 52, 63, 1.5).cut(cylinder(22.4, -2, 3))
    plate = plate.cut(box(-15, -34, -2, 30, 22, 3))
    for x in (-21, 21):
        for y in (-21, 21):
            plate = plate.cut(cylinder(1.15, -2, 3, x, y))
    keys = Part.makeCompound([box(k['x_mm']-(18+(k['width_u']-1)*19.05)/2, k['y_mm']-9, 5,
                                  18+(k['width_u']-1)*19.05, 18, 10) for k in layout['keys']])
    switches = Part.makeCompound([box(k['x_mm']-7, k['y_mm']-7, -9, 14, 14, 14) for k in layout['keys']])
    mounted = {n: p for n, p in parts.items() if n != 'SeatCoupon'}
    overlaps = {}
    for name, shape in parts.items():
        valid(shape, name)
    for (a, one), (b, two) in itertools.combinations(mounted.items(), 2):
        overlaps[a+'/'+b] = one.common(two).Volume
    for name, shape in mounted.items():
        for other, ref in {**references, 'PCB': pcb, 'CandidatePlate': plate, 'Keycaps': keys, 'Switches': switches}.items():
            overlaps[name+'/'+other] = shape.common(ref).Volume
    bad = {k: v for k, v in overlaps.items() if v > 1e-6}
    assert not bad, f'Nominal geometry collisions: {bad}'
    for offset in (-S['sensor_travel'], -1.5, 0., 1.5, S['sensor_travel']):
        for name in ('SensorCarriage', 'SensorCap', 'SensorBoardAssumption'):
            shape = (mounted | references)[name].copy()
            shape.translate(V(0, offset, 0))
            for other_name, other in dict(Cup=parts['Cup'], Ring=parts['RetainingRing'], PCB=pcb,
                                          Plate=plate, Keys=keys, Switches=switches, Ball=references['Ball']).items():
                assert shape.common(other).Volume < 1e-6, (name, other_name, 'travel collision', offset)
    # The ball can be dropped into the open cup; the installed ring retains it.
    for lift in (0., 2., 8., 18., 35.):
        raised = references['Ball'].copy()
        raised.translate(V(0, 0, lift))
        assert raised.common(parts['Cup']).Volume < 1e-6, 'Ball insertion obstructed'
    raised = references['Ball'].copy()
    raised.translate(V(0, 0, 3))
    assert raised.common(parts['RetainingRing']).Volume > 1., 'Ring fails to retain ball'
    assert parts['Cup'].isInside(V(0, 0, S['bottom_z']+1), 1e-7, True), 'Cup floor missing'
    references.update(CandidatePlate=plate, CurrentPCB=pcb)
    assembly = Part.makeCompound(list(mounted.values()))
    bb = assembly.BoundBox
    return dict(status='prototype_geometry_checked_not_hardware_qualified', settings_mm=S,
                coordinate_system='mm; ball XY=(0,0); plate top Z=0; rear=-Y',
                pcb_sha256=digest, layout_sha256=hashlib.sha256(layout_path.read_bytes()).hexdigest(),
                max_nominal_overlap_mm3=max(overlaps.values()),
                sensor_radial_travel_mm=[-S['sensor_travel'], S['sensor_travel']],
                ring_aperture_diameter_mm=2*math.sqrt((S['ball_radius']+S['cavity_clearance'])**2-(S['ring_top_z']-S['ball_z'])**2),
                printed_assembly_bounds_mm=[bb.XMin, bb.YMin, bb.ZMin, bb.XMax, bb.YMax, bb.ZMax],
                unverified=['sensor PCB/lens/components/connector envelope and optical datum',
                            'case depth, plate integration, fasteners, deflection',
                            'printed tolerances, bearing adhesion, optical tracking, cable routing'])


def preview(parts, references, path):
    # Orthographic mesh rendering avoids requiring a working OpenGL GUI.
    colors = {'Cup': '#168a83', 'RetainingRing': '#f4b451', 'SensorCarriage': '#4269a5',
              'SensorCap': '#244a82', 'Ball': '#dddddf', 'SensorBoardAssumption': '#6b9d70',
              'CeramicBall1': '#e4daab', 'CeramicBall2': '#e4daab', 'CeramicBall3': '#e4daab'}
    panels = []
    for exploded, title in ((False, 'ASSEMBLED'), (True, 'EXPLODED')):
        faces = []
        for name, color in colors.items():
            shape = (parts | references)[name].copy()
            if exploded:
                shift = {'RetainingRing': 36, 'Ball': 19, 'SensorCap': 15}.get(name, 0)
                shape.translate(V(0, 0, shift))
            mesh = MeshPart.meshFromShape(Shape=shape, LinearDeflection=.3, AngularDeflection=.35, Relative=False)
            vertices, triangles = mesh.Topology
            for tri in triangles:
                points = [vertices[i] for i in tri]
                normal = (points[1]-points[0]).cross(points[2]-points[0])
                view = V(1, -1.4, 1.1)
                if normal.dot(view) <= 0:
                    continue
                coords = [(p.x*.814+p.y*.581, p.x*.31-p.y*.435-p.z*.846) for p in points]
                depth = sum(p.dot(view) for p in points)/3
                shade = .65+.35*abs(normal.normalize().dot(V(.3, -.4, .866)))
                rgb = [int(int(color[i:i+2], 16)*shade) for i in (1, 3, 5)]
                paint = f'rgb({rgb[0]},{rgb[1]},{rgb[2]})'
                faces.append((depth, f'<polygon points="{" ".join(f"{x:.3f},{y:.3f}" for x,y in coords)}" fill="{paint}" stroke="{paint}" stroke-width=".04"/>'))
        offset = 290 if not exploded else 840
        panels.append(f'<text x="{offset}" y="65" text-anchor="middle">{title}</text><g transform="translate({offset},415) scale(4.6)">{"".join(f for _, f in sorted(faces))}</g>')
    path.write_text('<svg xmlns="http://www.w3.org/2000/svg" width="1140" height="650" viewBox="0 0 1140 650">'
                   '<rect width="1140" height="650" fill="#f6f5f1"/><g font-family="sans-serif" font-size="20" fill="#172033">'
                   +''.join(panels)+'<text x="35" y="590">34 mm ball / 62-key prototype / teal cup + amber ring + blue sensor carriage</text>'
                   '<text x="35" y="625" font-size="16">Sensor board is an unmeasured envelope. Plate, cable and fasteners omitted. NOT FIT-QUALIFIED.</text></g></svg>')


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    parts, references = geometry()
    checks = check(parts, references)
    doc = App.newDocument('PrintableHolderPrototype')
    objects = []
    artifacts = []
    for name, shape in (parts | references).items():
        obj = doc.addObject('Part::Feature', name)
        obj.Shape = shape
        obj.Visibility = name not in ('SeatCoupon', 'CandidatePlate', 'CurrentPCB')
        obj.addProperty('App::PropertyString', 'Scope').Scope = 'Independent prototype; dimensions edited in build_printable_holder.py'
        if name in parts:
            if name != 'SeatCoupon':
                objects.append(obj)
            print_shape = shape.copy()
            if name == 'RetainingRing':
                print_shape.rotate(V(0, 0, 0), V(1, 0, 0), 180)
            elif name in ('SensorCarriage', 'SensorCap'):
                print_shape.rotate(V(0, 0, 0), V(1, 0, 0), 90)
            bb = print_shape.BoundBox
            print_shape.translate(V(-bb.XMin, -bb.YMin, -bb.ZMin))
            mesh = MeshPart.meshFromShape(Shape=print_shape, LinearDeflection=.05, AngularDeflection=.15, Relative=False)
            assert mesh.isSolid() and mesh.countComponents() == 1, (name, 'non-manifold mesh')
            stl = OUT / (name+'-PROTOTYPE.stl')
            mesh.write(str(stl))
            loaded = Mesh.Mesh(str(stl))
            assert loaded.isSolid() and loaded.countComponents() == 1
            assert abs(abs(loaded.Volume)-print_shape.Volume)/print_shape.Volume < .005, (name, 'mesh volume drift')
            artifacts.append(stl)
    obj = doc.addObject('App::FeaturePython', 'DesignAssumptions')
    obj.addProperty('App::PropertyString', 'SettingsJSON').SettingsJSON = json.dumps(S)
    obj.addProperty('App::PropertyString', 'Unverified').Unverified = '; '.join(checks['unverified'])
    doc.recompute()
    fcstd = OUT / 'holder-PROTOTYPE.FCStd'
    doc.saveAs(str(fcstd))
    step = OUT / 'holder-PROTOTYPE.step'
    Part.export(objects, str(step))
    step.write_text('\n'.join(line.rstrip() for line in step.read_text().splitlines())+'\n')
    readback = Part.read(str(step))
    assert readback.isValid() and len(readback.Solids) == len(objects)
    expected_volume = sum(o.Shape.Volume for o in objects)
    # STEP reparameterizes trimmed spherical faces; allow one part per million.
    assert math.isclose(readback.Volume, expected_volume, rel_tol=1e-6, abs_tol=1e-3), (readback.Volume, expected_volume)
    checks['step_volume_relative_error'] = abs(readback.Volume-expected_volume)/expected_volume
    App.closeDocument(doc.Name)
    reopened = App.openDocument(str(fcstd))
    for name, shape in parts.items():
        valid(reopened.getObject(name).Shape, name+' FCStd readback')
        assert abs(reopened.getObject(name).Shape.Volume-shape.Volume) < 1e-6
    App.closeDocument(reopened.Name)
    preview(parts, references, OUT/'preview.svg')
    artifacts += [fcstd, step, OUT/'preview.svg']
    checks['artifact_sha256'] = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in artifacts}
    (OUT/'checks.json').write_text(json.dumps(checks, indent=2)+'\n')
    with zipfile.ZipFile(OUT/'holder-PROTOTYPE.zip', 'w', zipfile.ZIP_DEFLATED) as z:
        for path in artifacts+[OUT/'checks.json', OUT/'README.md']:
            z.write(path, path.name)
    print(json.dumps({'output': str(OUT), 'parts': list(parts), 'status': checks['status']}))


if __name__ == '__main__':
    main()
