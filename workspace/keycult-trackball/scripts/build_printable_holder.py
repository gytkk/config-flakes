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
S = dict(ball_radius=17., ball_z=8., bearing_radius=4., bearing_bore=3., bearing_width=4., cavity_clearance=.5,
         bottom_z=-14.,
         pcb_top_z=-5., pcb_thickness=1.6, plate_thickness=1.5,
         sensor_width=22., sensor_height=25., sensor_thickness=1.6,
         sensor_bottom_z=-17., sensor_travel=3., sensor_spacer=3., mount_pitch=42.,
         pilot_diameter=1.7, clearance_diameter=2.3)


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
    assert (s['bearing_bore'], s['bearing_radius'], s['bearing_width']) == (3., 4., 4.), 'Redesign cheeks and inner-ring spacers when changing the bearing'
    assert s['bottom_z'] < s['ball_z']-s['ball_radius']-1
    assert 0 < s['cavity_clearance'] < s['bearing_radius']
    center = V(0, 0, s['ball_z'])
    ball = Part.makeSphere(s['ball_radius'], center)
    cavity = Part.makeSphere(s['ball_radius']+s['cavity_clearance'], center)
    base = rounded_box((-22, -22, 22, 22), 3, s['bottom_z'], 2).cut(cylinder(12, -15, 4))
    references = {'Ball': ball}
    # Independent 3x8x4mm roller selection; tangential horizontal axes give 45-degree contacts.
    for angle in (90, 210, 330):
        a = math.radians(angle)
        radial = (s['ball_radius']+s['bearing_radius'])/math.sqrt(2)
        p = V(radial*math.cos(a), radial*math.sin(a), s['ball_z']-radial)
        axis = V(-math.sin(a), math.cos(a), 0)
        bearing = Part.makeCylinder(s['bearing_radius'], 4, p-axis*2, axis).cut(Part.makeCylinder(1.5, 4, p-axis*2, axis))
        references[f'Roller{angle}'] = bearing
        for side in (-1, 1):
            # 0.3mm metal inner-ring spacers prevent the cheeks rubbing the outer ring.
            spacer_start = -2.3 if side == -1 else 2.
            spacer = Part.makeCylinder(2, .3, p+axis*spacer_start, axis).cut(Part.makeCylinder(1.55, .3, p+axis*spacer_start, axis))
            references[f'InnerSpacer{angle}_{side}'] = spacer
            cheek = box(radial-2.5, -5.3 if side == -1 else 2.3, -14, 5, 3, p.z+3+14)
            cheek.rotate(V(0, 0, 0), V(0, 0, 1), angle)
            cheek = cheek.cut(Part.makeCylinder(1.6, 20, p-axis*10, axis)).cut(cavity)
            base = base.fuse(cheek)
        references[f'Axle{angle}'] = Part.makeCylinder(1.5, 14, p-axis*5.3, axis)
        assert abs((p-center).Length-s['ball_radius']-s['bearing_radius']) < 1e-8
        assert ball.common(bearing).Volume < 1e-6 and ball.distToShape(bearing)[0] < 1e-6
    # Four posts join the cradle to the plate; separate bosses carry the sensor below it.
    for x in (-s['mount_pitch']/2, s['mount_pitch']/2):
        for y in (-s['mount_pitch']/2, s['mount_pitch']/2):
            base = base.fuse(cylinder(2.5, -14, 12.5, x, y))
    for x in (-s['mount_pitch']/2, s['mount_pitch']/2):
        for y in (-s['mount_pitch']/2, s['mount_pitch']/2):
            base = base.cut(cylinder(s['clearance_diameter']/2, -15, 15, x, y))
    z0 = s['sensor_bottom_z']
    top = z0+s['sensor_thickness']+.2
    carriage = rounded_box((-22, -22, 22, 22), 3, z0-2, 2).cut(box(-10, -11.5, z0-3, 20, 23, 4))
    locators = box(-13, -14.5, z0, 26, 29, s['sensor_thickness']).cut(box(-11.2, -12.7, z0-1, 22.4, 25.4, 4))
    carriage = carriage.fuse(locators)
    cap = rounded_box((-16, -16, 16, 16), 2, top, 1).cut(box(-10, -11.5, top-1, 20, 23, 3))
    for x in (-14., 14.):
        carriage = carriage.fuse(cylinder(2.5, z0, top-z0, x))
        carriage = carriage.cut(cylinder(s['pilot_diameter']/2, z0-3, 7, x))
        cap = cap.cut(cylinder(s['clearance_diameter']/2, top-1, 3, x))
    spacers = {}
    for x in (-18., 18.):
        for y in (-18., 18.):
            base = base.fuse(cylinder(3, -14, 6, x, y)).cut(cylinder(s['pilot_diameter']/2, -15, 8, x, y))
            carriage = carriage.cut(cylinder(s['clearance_diameter']/2, z0-3, 4, x, y))
            spacers[f'SensorSpacer{len(spacers)+1}'] = cylinder(2.2, z0, s['sensor_spacer'], x, y).cut(cylinder(1.15, z0-1, 5, x, y))
    sensor = box(-s['sensor_width']/2, -s['sensor_height']/2, z0,
                 s['sensor_width'], s['sensor_height'], s['sensor_thickness'])
    references['SensorBoardAssumption'] = sensor
    # These are fit coupons, not promises about a printing process's bore tolerance.
    coupon = box(0, 0, 0, 30, 12, 4)
    for x, radius in ((5., 1.5), (15., 1.55), (25., 1.6)):
        coupon = coupon.cut(cylinder(radius, -1, 6, x, 4))
    coupon = coupon.cut(cylinder(s['pilot_diameter']/2, -1, 6, 10, 9))
    coupon = coupon.cut(cylinder(s['clearance_diameter']/2, -1, 6, 20, 9))
    return {"Cup": base.removeSplitter(),
            "SensorCarriage": carriage.removeSplitter(), "SensorCap": cap.removeSplitter(),
            "SeatCoupon": coupon.removeSplitter(), **spacers}, references


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
    plate = box(-26, -26, -1.5, 52, 52, 1.5).cut(cylinder(18, -2, 3))
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
    pcb_gap = min(shape.distToShape(pcb)[0] for shape in mounted.values())
    assert pcb_gap >= .49, ('PCB passage clearance below 0.5mm nominal', pcb_gap)
    for dx, dy in ((.5, 0), (-.5, 0), (0, .5), (0, -.5)):
        displaced = references['SensorBoardAssumption'].copy()
        displaced.translate(V(dx, dy, 0))
        assert displaced.common(parts['SensorCarriage']).Volume > 1e-6, 'Sensor XY locators missing'
    for offset in (-S['sensor_travel'], -1.5, 0.):
        adjusted = {}
        for name in ('SensorCarriage', 'SensorCap', 'SensorBoardAssumption'):
            shape = (mounted | references)[name].copy()
            shape.translate(V(0, 0, offset))
            adjusted[name] = shape
        for i, (x, y) in enumerate(itertools.product((-18., 18.), repeat=2), 1):
            z = S['sensor_bottom_z']+offset
            height = S['sensor_spacer']-offset
            adjusted[f'SensorSpacer{i}'] = cylinder(2.2, z, height, x, y).cut(cylinder(1.15, z-1, height+2, x, y))
        for name, shape in adjusted.items():
            for other_name, other in dict(Cup=parts['Cup'], PCB=pcb,
                                          Plate=plate, Keys=keys, Switches=switches, Ball=references['Ball']).items():
                assert shape.common(other).Volume < 1e-6, (name, other_name, 'travel collision', offset)
        for (a, one), (b, two) in itertools.combinations(adjusted.items(), 2):
            assert one.common(two).Volume < 1e-6, ('adjusted assembly collision', a, b, offset)
    # The user accepts lift-out; no printed part may obstruct vertical removal.
    for lift in (0., 2., 3., 8., 18., 35.):
        raised = references['Ball'].copy()
        raised.translate(V(0, 0, lift))
        for name, shape in mounted.items():
            assert raised.common(shape).Volume < 1e-6, (name, 'Ball insertion/removal obstructed', lift)
    assert parts['Cup'].isInside(V(20, 0, S['bottom_z']+1), 1e-7, True), 'Cradle floor missing'
    # The central optical path must remain open through both printed supports.
    optical = cylinder(5.5, -21, 12)
    assert all(shape.common(optical).Volume < 1e-6 for shape in mounted.values()), 'Optical path obstructed'
    references.update(CandidatePlate=plate, CurrentPCB=pcb)
    assembly = Part.makeCompound(list(mounted.values()))
    bb = assembly.BoundBox
    return dict(status='prototype_geometry_checked_not_hardware_qualified', settings_mm=S,
                coordinate_system='mm; ball XY=(0,0); plate top Z=0; rear=-Y',
                pcb_sha256=digest, layout_sha256=hashlib.sha256(layout_path.read_bytes()).hexdigest(),
                max_nominal_overlap_mm3=max(overlaps.values()),
                printed_parts_to_pcb_minimum_gap_mm=pcb_gap,
                sensor_vertical_shim_range_mm=[-S['sensor_travel'], 0],
                ball_retention='none; vertical lift-out is intentional',
                printed_assembly_bounds_mm=[bb.XMin, bb.YMin, bb.ZMin, bb.XMax, bb.YMax, bb.ZMax],
                unverified=['sensor PCB/lens/components/connector envelope and optical datum',
                            'case depth, plate integration, fasteners, deflection',
                            'printed tolerances, inner-ring spacers and axle fit, optical tracking, cable routing'])


def preview(parts, references, path):
    # Orthographic mesh rendering avoids requiring a working OpenGL GUI.
    colors = {name: '#d8bc76' for name in (parts | references) if name.startswith(('Roller', 'Axle', 'InnerSpacer', 'SensorSpacer'))}
    colors.update(Cup='#168a83', SensorCarriage='#4269a5', SensorCap='#244a82', Ball='#dddddf', SensorBoardAssumption='#6b9d70')
    panels = []
    for exploded, title in ((False, 'ASSEMBLED'), (True, 'EXPLODED')):
        faces = []
        for name, color in colors.items():
            shape = (parts | references)[name].copy()
            if exploded:
                shift = {'Ball': 19, 'SensorCarriage': -12, 'SensorCap': -6, 'SensorBoardAssumption': -9}.get(name, 0)
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
                   +''.join(panels)+'<text x="35" y="590">34 mm lift-out ball / three rollers / horizontal under-ball sensor</text>'
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
