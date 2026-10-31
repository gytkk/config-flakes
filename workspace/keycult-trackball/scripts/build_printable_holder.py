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
S = dict(ball_radius=17., ball_z=23.5, bearing_radius=4., bearing_bore=3., bearing_width=4., cavity_clearance=.5,
         bottom_z=1.5,
         pcb_top_z=-5., pcb_thickness=1.6, plate_thickness=1.5,
         sensor_width=22., sensor_height=25., sensor_thickness=1.6,
         sensor_bottom_z=-3., sensor_travel=1.5, sensor_spacer=4.5, mount_pitch=42.,
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
    assert math.isclose(s['sensor_bottom_z']+s['sensor_spacer'], s['bottom_z']), 'Sensor spacers must contact the cradle underside'
    center = V(0, 0, s['ball_z'])
    ball = Part.makeSphere(s['ball_radius'], center)
    cavity = Part.makeSphere(s['ball_radius']+s['cavity_clearance'], center)
    base = rounded_box((-22, -22, 22, 22), 3, s['bottom_z'], 2).cut(cylinder(12, s['bottom_z']-1, 4))
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
            cheek = box(radial-2.5, -5.3 if side == -1 else 2.3, s['bottom_z'], 5, 3, p.z+3-s['bottom_z'])
            cheek.rotate(V(0, 0, 0), V(0, 0, 1), angle)
            cheek = cheek.cut(Part.makeCylinder(1.6, 20, p-axis*10, axis)).cut(cavity)
            base = base.fuse(cheek)
        references[f'Axle{angle}'] = Part.makeCylinder(1.5, 14, p-axis*5.3, axis)
        references[f'AxleHead{angle}'] = Part.makeCylinder(2.75, 3, p-axis*8.3, axis)
        references[f'AxleNutEnvelope{angle}'] = Part.makeCylinder(3.2, 2.4, p+axis*5.3, axis).cut(Part.makeCylinder(1.5, 2.4, p+axis*5.3, axis))
        assert abs((p-center).Length-s['ball_radius']-s['bearing_radius']) < 1e-8
        assert ball.common(bearing).Volume < 1e-6 and ball.distToShape(bearing)[0] < 1e-6
    # The shallow cradle rests on the plate top instead of hanging below it.
    for x in (-s['mount_pitch']/2, s['mount_pitch']/2):
        for y in (-s['mount_pitch']/2, s['mount_pitch']/2):
            base = base.fuse(cylinder(2.5, 0, s['bottom_z']+2, x, y))
    for x in (-s['mount_pitch']/2, s['mount_pitch']/2):
        for y in (-s['mount_pitch']/2, s['mount_pitch']/2):
            base = base.cut(cylinder(s['clearance_diameter']/2, -1, s['bottom_z']+4, x, y))
    z0 = s['sensor_bottom_z']
    top = z0+s['sensor_thickness']+.2
    carriage = rounded_box((-18, -18, 18, 18), 3, z0-2, 2).cut(box(-10, -11.5, z0-3, 20, 23, 4))
    locators = box(-13, -14.5, z0, 26, 29, s['sensor_thickness']).cut(box(-11.2, -12.7, z0-1, 22.4, 25.4, 4))
    carriage = carriage.fuse(locators)
    cap = rounded_box((-16, -16, 16, 16), 2, top, 1).cut(box(-10, -11.5, top-1, 20, 23, 3))
    for x in (-14., 14.):
        carriage = carriage.fuse(cylinder(2.5, z0, top-z0, x))
        carriage = carriage.cut(cylinder(s['pilot_diameter']/2, z0-3, 7, x))
        cap = cap.cut(cylinder(s['clearance_diameter']/2, top-1, 3, x))
        # Cap screw heads need through-reliefs in the cradle floor, not hidden collisions.
        base = base.cut(cylinder(2.2, s['bottom_z']-.1, 2.2, x))
        references[f'CapScrewHead{x:g}'] = cylinder(1.9, top+1, 2, x)
        references[f'CapScrewShaft{x:g}'] = cylinder(1, top+1-5, 5, x)
    spacers = {}
    for x in (-14., 14.):
        for y in (-14., 14.):
            # Lower bosses leave a straight hex-key approach to the roller axles.
            base = base.fuse(cylinder(3, s['bottom_z'], 4, x, y)).cut(cylinder(s['pilot_diameter']/2, s['bottom_z']-1, 6, x, y))
            carriage = carriage.cut(cylinder(s['clearance_diameter']/2, z0-3, 4, x, y))
            carriage = carriage.cut(cylinder(2.4, z0, 4, x, y))
            cap = cap.cut(cylinder(2.4, top-1, 3, x, y))
            spacers[f'SensorSpacer{len(spacers)+1}'] = cylinder(2.2, z0, s['sensor_spacer'], x, y).cut(cylinder(1.15, z0-1, s['sensor_spacer']+2, x, y))
            references[f'SensorScrewHead{x:g}_{y:g}'] = cylinder(1.9, z0-4, 2, x, y)
            references[f'SensorScrewShaft{x:g}_{y:g}'] = cylinder(1, z0-2, 10, x, y)
    for x, y in itertools.product((-21., 21.), repeat=2):
        # Insert from underneath so the wider nut does not foul the sensor spacers.
        references[f'PlateScrewHead{x:g}_{y:g}'] = cylinder(1.9, -3.5, 2, x, y)
        references[f'PlateScrewShaft{x:g}_{y:g}'] = cylinder(1, -1.5, 8, x, y)
        references[f'PlateNutEnvelope{x:g}_{y:g}'] = cylinder(2.35, s['bottom_z']+2, 1.6, x, y).cut(cylinder(1, s['bottom_z']+2, 1.6, x, y))
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
    stack = layout['prototype_stack_mm']
    assert (stack['ball_center_z'], stack['pcb_top_z'], stack['pcb_thickness'], stack['assumed_floor_z']) == (S['ball_z'], S['pcb_top_z'], S['pcb_thickness'], -7.42), 'Prototype stack differs from the layout source'
    assert report['artifact_sha256'][pcb_path.name] == digest, 'Revalidate modified native PCB first'
    assert report['pcb_opening'] == layout['pcb_opening'], 'Layout/PCB opening mismatch'
    bounds = report['envelope_bounds_mm']
    pcb = box(bounds[0]-180, bounds[1]-100, S['pcb_top_z']-S['pcb_thickness'],
              bounds[2]-bounds[0], bounds[3]-bounds[1], S['pcb_thickness'])
    hole = report['pcb_opening']
    pcb = pcb.cut(rounded_box(hole['bounds_mm'], hole['corner_radius_mm'], -8, 5))
    # Generic local plate coupon only; no upstream outline is reproduced.
    plate = box(-26, -26, -1.5, 52, 52, 1.5).cut(rounded_box((-18, -18, 18, 18), 3, -2, 3))
    for x in (-21, 21):
        for y in (-21, 21):
            plate = plate.cut(cylinder(1.15, -2, 3, x, y))
    keys = Part.makeCompound([box(k['x_mm']-(18+(k['width_u']-1)*19.05)/2, k['y_mm']-9, 5,
                                  18+(k['width_u']-1)*19.05, 18, 10) for k in layout['keys']])
    switches = Part.makeCompound([box(k['x_mm']-7.8, k['y_mm']-7.8, -9, 15.6, 15.6, 14) for k in layout['keys']])
    mounted = {n: p for n, p in parts.items() if n != 'SeatCoupon'}
    overlaps = {}
    for name, shape in parts.items():
        valid(shape, name)
    for (a, one), (b, two) in itertools.combinations(mounted.items(), 2):
        overlaps[a+'/'+b] = one.common(two).Volume
    for name, shape in mounted.items():
        for other, ref in {**references, 'PCB': pcb, 'CandidatePlate': plate, 'Keycaps': keys, 'Switches': switches}.items():
            common = shape.common(ref)
            # M2 shafts intentionally engage the 1.7mm pilot holes in these two parts.
            if name == 'Cup' and other.startswith('SensorScrewShaft'):
                common = common.cut(box(-25, -25, S['bottom_z'], 50, 50, 4))
            if name == 'SensorCarriage' and other.startswith('CapScrewShaft'):
                common = common.cut(box(-25, -25, S['sensor_bottom_z']-2, 50, 50, 4))
            overlaps[name+'/'+other] = common.Volume
    hardware = {n: p for n, p in references.items() if n != 'SensorBoardAssumption'}
    for (a, one), (b, two) in itertools.combinations(hardware.items(), 2):
        overlaps[a+'/'+b] = one.common(two).Volume
    for name, shape in references.items():
        for other, ref in dict(PCB=pcb, CandidatePlate=plate, Keycaps=keys, Switches=switches).items():
            overlaps[name+'/'+other] = shape.common(ref).Volume
    bad = {k: v for k, v in overlaps.items() if v > 1e-6}
    assert not bad, f'Nominal geometry collisions: {bad}'
    pcb_gap = min(shape.distToShape(pcb)[0] for shape in mounted.values())
    assert pcb_gap >= .49, ('PCB passage clearance below 0.5mm nominal', pcb_gap)
    for dx, dy in ((.5, 0), (-.5, 0), (0, .5), (0, -.5)):
        displaced = references['SensorBoardAssumption'].copy()
        displaced.translate(V(dx, dy, 0))
        assert displaced.common(parts['SensorCarriage']).Volume > 1e-6, 'Sensor XY locators missing'
    travel_hardware = []
    for offset in (0., S['sensor_travel']/2, S['sensor_travel']):
        adjusted = {}
        for name in ('SensorCarriage', 'SensorCap', 'SensorBoardAssumption',
                     *(n for n in references if n.startswith(('CapScrew', 'SensorScrewHead')))):
            shape = (mounted | references)[name].copy()
            shape.translate(V(0, 0, offset))
            adjusted[name] = shape
        for i, (x, y) in enumerate(itertools.product((-14., 14.), repeat=2), 1):
            z = S['sensor_bottom_z']+offset
            height = S['sensor_spacer']-offset
            adjusted[f'SensorSpacer{i}'] = cylinder(2.2, z, height, x, y).cut(cylinder(1.15, z-1, height+2, x, y))
            length = 8 if offset == S['sensor_travel'] else 10
            adjusted[f'SensorScrewShaft{x:g}_{y:g}'] = cylinder(1, z-2, length, x, y)
        engagement = min(S['bottom_z']+4, z-2+length)-S['bottom_z']
        assert engagement >= 3., ('Sensor screw engagement below prototype 3mm criterion', offset, engagement)
        travel_hardware.append(dict(offset_mm=offset, spacer_length_mm=height, sensor_screw_length_mm=length, pilot_engagement_mm=engagement))
        for name, shape in adjusted.items():
            for other_name, other in dict(Cup=parts['Cup'], PCB=pcb,
                                          Plate=plate, Keys=keys, Switches=switches, Ball=references['Ball']).items():
                common = shape.common(other)
                if name.startswith('SensorScrewShaft') and other_name == 'Cup':
                    common = common.cut(box(-25, -25, S['bottom_z'], 50, 50, 4))
                assert common.Volume < 1e-6, (name, other_name, 'travel collision', offset)
        for (a, one), (b, two) in itertools.combinations(adjusted.items(), 2):
            if (a == 'SensorCarriage' and b.startswith('CapScrewShaft')
                    or b == 'SensorCarriage' and a.startswith('CapScrewShaft')):
                continue
            assert one.common(two).Volume < 1e-6, ('adjusted assembly collision', a, b, offset)
    # The user accepts lift-out; no printed part may obstruct vertical removal.
    for lift in (0., 2., 3., 8., 18., 35.):
        raised = references['Ball'].copy()
        raised.translate(V(0, 0, lift))
        for name, shape in (mounted | {n: p for n, p in references.items() if n != 'Ball'}).items():
            assert raised.common(shape).Volume < 1e-6, (name, 'Ball insertion/removal obstructed', lift)
    assert parts['Cup'].isInside(V(20, 0, S['bottom_z']+1), 1e-7, True), 'Cradle floor missing'
    # The central optical path must remain open through both printed supports.
    optical = cylinder(5.5, S['sensor_bottom_z']-4, S['ball_z']-S['ball_radius']-S['sensor_bottom_z']+4)
    assert all(shape.common(optical).Volume < 1e-6 for shape in mounted.values()), 'Optical path obstructed'
    for offset in (-20., -10., -3., -1., 0.):
        for name in ('SensorCarriage', 'SensorCap', 'SensorBoardAssumption',
                     *(n for n in references if n.startswith(('CapScrew', 'SensorScrew')))):
            moving = (mounted | references)[name].copy()
            moving.translate(V(0, 0, offset))
            assert moving.common(plate).Volume < 1e-6, ('Sensor assembly blocked by plate during insertion from below', name, offset)
        board = pcb.copy()
        board.translate(V(0, 0, offset))
        assert all(board.common(p).Volume < 1e-6 for p in (mounted | references).values()), 'PCB installation path blocked'
    tool_gaps = {}
    for angle in (90, 210, 330):
        a = math.radians(angle)
        axis = V(-math.sin(a), math.cos(a), 0)
        radial = (S['ball_radius']+S['bearing_radius'])/math.sqrt(2)
        p = V(radial*math.cos(a), radial*math.sin(a), S['ball_z']-radial)
        driver = Part.makeCylinder(1.5, 20, p-axis*28.3, axis)
        assert driver.common(parts['Cup']).Volume < 1e-6, ('Axle hex-key approach blocked', angle)
        tool_gaps[f'axle_{angle}'] = driver.distToShape(parts['Cup'])[0]
    for x in (-14., 14.):
        driver = cylinder(1.5, S['sensor_bottom_z']+S['sensor_thickness']+3.2, 30, x)
        for name, shape in dict(Cup=parts['Cup'], Plate=plate, PCB=pcb).items():
            assert driver.common(shape).Volume < 1e-6, ('Cap driver approach blocked with ball removed', x, name)
        tool_gaps[f'cap_{x:g}'] = driver.distToShape(parts['Cup'])[0]
    references.update(CandidatePlate=plate, CurrentPCB=pcb)
    assembly = Part.makeCompound(list(mounted.values()))
    bb = assembly.BoundBox
    minimum_z = min(p.BoundBox.ZMin for p in (mounted | references).values())
    assert minimum_z >= -7.42, ('Below user-assumed Mill-Max 3305-0 floor', minimum_z)
    return dict(status='prototype_geometry_checked_not_hardware_qualified', settings_mm=S,
                coordinate_system='mm; ball XY=(0,0); plate top Z=0; rear=-Y',
                pcb_sha256=digest, layout_sha256=hashlib.sha256(layout_path.read_bytes()).hexdigest(),
                max_nominal_overlap_mm3=max(overlaps.values()),
                printed_parts_to_pcb_minimum_gap_mm=pcb_gap,
                sensor_vertical_shim_range_mm=[0, S['sensor_travel']],
                sensor_travel_hardware=travel_hardware, driver_shaft_radius_checked_mm=1.5,
                driver_approach_to_cup_minimum_gap_mm=tool_gaps,
                ball_retention='none; vertical lift-out is intentional',
                printed_assembly_bounds_mm=[bb.XMin, bb.YMin, bb.ZMin, bb.XMax, bb.YMax, bb.ZMax],
                required_depth_with_candidate_screw_heads_mm=7.,
                lowest_modeled_z_mm=minimum_z, assumed_floor_z_mm=-7.42,
                floor_check_scope='Holder, candidate hardware, sensor-board envelope and bare main PCB only; keyswitch pins, plastic bosses and solder are not modeled',
                nominal_clearance_to_assumed_floor_mm=minimum_z+7.42,
                protrusion_below_assumed_main_pcb_bottom_mm=.4,
                candidate_hardware='M3x14 socket screws, M3 nut envelopes; underside-inserted M2x8 plate screws; M2x10 sensor and M2x5 cap socket screws',
                plate_head_to_sensor_spacer_gap_mm=min(references['PlateScrewHead21_21'].distToShape(p)[0] for n,p in mounted.items() if n.startswith('SensorSpacer')),
                unverified=['sensor PCB/lens/components/connector envelope and optical datum',
                            'case depth, actual fastener tolerances, driver access in case, deflection',
                            'printed tolerances, inner-ring spacers and axle fit, optical tracking, cable routing'])


def preview(parts, references, path):
    # Orthographic mesh rendering avoids requiring a working OpenGL GUI.
    colors = {name: '#d8bc76' for name in (parts | references) if name.startswith(('Roller', 'Axle', 'InnerSpacer', 'SensorSpacer', 'CapScrew', 'SensorScrew', 'PlateScrew', 'PlateNut'))}
    colors.update(Cup='#168a83', SensorCarriage='#4269a5', SensorCap='#244a82', Ball='#dddddf', SensorBoardAssumption='#6b9d70')
    panels = []
    for exploded, title in ((False, 'ASSEMBLED'), (True, 'EXPLODED')):
        faces = []
        for name, color in colors.items():
            shape = (parts | references)[name].copy()
            if exploded:
                shift = {'Ball': 19, 'SensorCarriage': -12, 'SensorCap': -6, 'SensorBoardAssumption': -9}.get(name, 0)
                if name.startswith('CapScrew'):
                    shift = -6
                if name.startswith('SensorScrew'):
                    shift = -12
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
                   '<text x="35" y="625" font-size="16">Shallow prototype: lowest hardware Z=-7; ball top Z=40.5. Sensor, case and cable unmeasured.</text></g></svg>')


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
