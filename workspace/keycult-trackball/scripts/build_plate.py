# /// script
# requires-python = ">=3.11"
# dependencies = ["ezdxf==1.4.3"]
# ///
"""Build local-only plate studies with the installed FreeCAD Python runtime."""
import hashlib
import json
import math
import os
from pathlib import Path
import sys
import zipfile

import ezdxf

ROOT = Path(__file__).resolve().parents[1]
APP = Path(os.environ.get("FREECAD_ROOT", "/Applications/FreeCAD.app/Contents/Resources"))
sys.path.insert(0, str(APP / "lib"))
try:
    import FreeCAD as App
    import Part
except ImportError as error:
    raise SystemExit("Use uv run --python <FreeCAD Resources/bin/python> scripts/build_plate.py; set FREECAD_ROOT if needed") from error

from fetch_sources import SOURCES

LOCAL = ROOT / "design/preliminary/sources/local/plate-study"
UPSTREAM = ROOT / "design/preliminary/sources/upstream"
STEP_SHA = "bf236229c06eff56717782319b435723cf583641a0352cc77b17366bb97ccd48"
# These are study choices, not measured Keyball dimensions or machining tolerances.
STUDY = dict(plate_thickness=1.5, switch_opening=14.0, opening_diameter=40.0,
             carrier_width=48.0, carrier_thickness=3.0, mount_pitch=42.0,
             mount_hole_diameter=2.2, pcb_top_z=-5.0, pcb_thickness=1.6,
             ball_center_z=8.0, ball_radius=17.0, alternative_ball_center_z=13.0,
             rear_extension_y=-40.0, rear_shelf_width=30.0)
V = App.Vector


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def rect(x, y, w, h, z=0):
    return Part.Face(Part.makePolygon([V(x,y,z), V(x+w,y,z), V(x+w,y+h,z), V(x,y+h,z), V(x,y,z)]))


def pcb_report(count, variant):
    directory = ROOT / 'design/pcb' / ('62' if count == 62 else '')
    path = directory / f'keycult{count}.kicad_pcb'
    report = json.loads((directory / 'reports/validation.json').read_text())
    if sha(path) != report['artifact_sha256'][path.name]:
        raise ValueError('PCB has changed since native validation; rerun validate_pcb.py')
    if (report['key_count'] != count or report['layout_variant'] != ('preferred' if count == 65 else 'rear_clearance')
            or report['structural_errors'] or report.get('pcb_opening') != variant.get('pcb_opening')):
        raise ValueError('Validated PCB geometry differs from layout; rerun validate_pcb.py')
    return path, report


def pcb_shape(report, top_z, thickness):
    xmin, ymin, xmax, ymax = report['envelope_bounds_mm']
    z = top_z-thickness
    board = Part.makeBox(xmax-xmin, ymax-ymin, thickness, V(xmin-180, ymin-100, z))
    opening = report.get('pcb_opening')
    if opening:
        x0, y0, x1, y1 = opening['bounds_mm']
        radius = opening['corner_radius_mm']
        w, h = x1-x0, y1-y0
        if not all(math.isfinite(v) for v in (x0, y0, x1, y1, radius)) or not 0 < 2*radius < min(w, h):
            raise ValueError('Invalid rounded PCB opening dimensions')
        tool = Part.makeBox(w-2*radius, h, thickness, V(x0+radius, y0, z)).fuse(
            Part.makeBox(w, h-2*radius, thickness, V(x0, y0+radius, z)))
        for x in (x0+radius, x1-radius):
            for y in (y0+radius, y1-radius):
                tool = tool.fuse(Part.makeCylinder(radius, thickness, V(x, y, z)))
        tool = tool.removeSplitter()
        expected_volume = (w*h-(4-math.pi)*radius**2)*thickness
        if not tool.isValid() or len(tool.Solids) != 1 or abs(tool.Volume-expected_volume) > 1e-5:
            raise ValueError('Rounded PCB opening reconstruction failed')
        opened = board.cut(tool).removeSplitter()
        if abs(board.Volume-opened.Volume-expected_volume) > 1e-5:
            raise ValueError('PCB opening is not fully inside the native envelope')
        board = opened
        assert not board.isInside(V((x0+x1)/2, (y0+y1)/2, z+thickness/2), 1e-7, True), 'PCB opening center is filled'
        assert board.isInside(V(x0+radius/10, y0+radius/10, z+thickness/2), 1e-7, True), 'Rounded PCB corner was removed'
    if not board.isValid() or len(board.Solids) != 1 or board.Volume <= 0:
        raise ValueError('Expected one valid PCB reference solid')
    return board


def feature(doc, name, shape, note):
    obj = doc.addObject("Part::Feature", name)
    obj.Label = name
    obj.Shape = shape
    obj.addProperty("App::PropertyString", "Scope", "Study").Scope = note
    return obj


def cut(doc, name, base, tool):
    obj = doc.addObject("Part::Cut", name)
    obj.Base, obj.Tool = base, tool
    obj.Refine = True
    doc.recompute()
    return obj


def source_blank():
    archive = UPSTREAM / "No160-Rev1-Plates.zip"
    if sha(archive) != SOURCES[archive.name][1]:
        raise ValueError("Official archive hash differs; inspect upstream before rebuilding")
    with zipfile.ZipFile(archive) as z:
        data = z.read("Plate-NoLeaf.step")
    if hashlib.sha256(data).hexdigest() != STEP_SHA:
        raise ValueError("NoLeaf STEP hash differs")
    step = UPSTREAM / "Plate-NoLeaf.step"
    step.write_bytes(data)
    source = Part.read(str(step))
    if not source.isValid() or len(source.Solids) != 1:
        raise ValueError("Expected one valid NoLeaf solid")
    top = max((f for f in source.Faces if abs(f.CenterOfMass.y) < 1e-6 and abs(f.normalAt(0,0).y) > .99), key=lambda f:f.Area)
    inner = [w for w in top.Wires if not w.isSame(top.OuterWire)]
    # The pinned NoLeaf face has 68 key/stabilizer openings, all in the keyfield.
    if len(inner) != 68 or any(w.BoundBox.XMin < 2 or w.BoundBox.XMax > 284 or w.BoundBox.ZMin < 2 or w.BoundBox.ZMax > 94 for w in inner):
        raise ValueError("Source internal features changed; classify mounting features before replacing holes")
    matrix = App.Matrix()
    matrix.A11, matrix.A22, matrix.A23, matrix.A32, matrix.A33 = 1, 0, 1, 1, 0
    matrix.A14, matrix.A24 = -142.875, -47.625
    outline = top.OuterWire.transformGeometry(matrix)
    face = Part.Face(outline)
    blank = face.extrude(V(0,0,-STUDY['plate_thickness']))
    donor = source.transformGeometry(matrix)
    if not blank.isValid() or len(blank.Solids) != 1:
        raise ValueError("Reconstructed blank is invalid")
    return face, blank, donor


def write_dxf(face, path):
    drawing = ezdxf.new("R2010")
    drawing.units = 4
    drawing.layers.new("DRAFT_CUT")
    for wire in face.Wires:
        points = wire.discretize(Deflection=.01)
        drawing.modelspace().add_lwpolyline([(p.x,p.y) for p in points], close=True, dxfattribs={"layer":"DRAFT_CUT"})
    drawing.saveas(path)
    reopened = ezdxf.readfile(path)
    if reopened.header['$INSUNITS'] != 4 or len(reopened.modelspace()) != len(face.Wires):
        raise ValueError("DXF readback failed")


def wire_path(wire):
    points = wire.discretize(Deflection=.02)
    return 'M' + ' L'.join(f'{p.x:.4f},{p.y:.4f}' for p in points) + ' Z'


def cap_width(key):
    return 18 + (key['width_u'] - 1) * 19.05


def svg(face, carrier_face, keys, count, path, pcb_opening):
    s = STUDY
    paths = ' '.join(wire_path(w) for w in face.Wires)
    frame = ' '.join(wire_path(w) for w in carrier_face.Wires)
    caps = ''.join(f'<rect x="{k["x_mm"]-cap_width(k)/2}" y="{k["y_mm"]-9}" width="{cap_width(k)}" height="18" fill="none" stroke="#64748b" stroke-width=".15" stroke-dasharray="1 1"/>' for k in keys)
    path.write_text(f'''<svg xmlns="http://www.w3.org/2000/svg" width="330mm" height="170mm" viewBox="-165 -85 330 170">
<rect x="-165" y="-85" width="330" height="170" fill="white"/>
<g font-family="sans-serif" fill="#172033"><text x="-155" y="-73" font-size="5">No.1/60 NoLeaf / {count} keys / LOCAL MECHANICAL STUDY</text>
<text x="-155" y="-65" font-size="3">NOT FOR FABRICATION: revision, switch fit, holder and Z stack unverified</text></g>
<path d="{paths}" fill="#e2e8f0" fill-rule="evenodd" stroke="#334155" stroke-width=".25"/>{caps}
<path d="{frame}" fill="#99f6e4" fill-opacity=".7" fill-rule="evenodd" stroke="#0f766e" stroke-width=".3"/>
<circle cx="0" cy="0" r="{s['ball_radius']:g}" fill="#fdba74" fill-opacity=".6" stroke="#ea580c" stroke-width=".3"/>
<g font-family="sans-serif" font-size="3"><text x="-155" y="66">Orange: {2*s['ball_radius']:g}mm ball projection. Teal: proposed carrier, not a Keyball holder.</text>
<text x="-155" y="72">Plate opening D{s['opening_diameter']:g} / carrier {s['carrier_width']:g}x{s['carrier_width']:g}x{s['carrier_thickness']:g} / 4x D{s['mount_hole_diameter']:g} on {s['mount_pitch']:g}mm square: study assumptions.</text>
<text x="-155" y="78">PCB: {'native rounded central opening' if pcb_opening else 'uncut'}. Ball center Z={s['ball_center_z']:g}; PCB top Z={s['pcb_top_z']:g} assumed. See interference report.</text></g></svg>''')


def build(count, layout, face, blank, donor):
    s = STUDY
    t, half, radius = s['plate_thickness'], s['carrier_width']/2, s['opening_diameter']/2
    variant = 'preferred' if count == 65 else 'rear_clearance'
    keys = layout[variant]['keys']
    key_count = len(keys)
    directory = ROOT / 'design/pcb' / ('62' if count == 62 else '')
    pcb, report = pcb_report(count, layout[variant])
    coordinates = json.loads((directory/'reports/switch-coordinates.json').read_text())
    actual = {k['id']:[k['actual_stem_mm'][0]-180,k['actual_stem_mm'][1]-100] for k in coordinates}
    if set(actual) != {k['id'] for k in keys} or any(math.dist(actual[k['id']],[k['x_mm'],k['y_mm']]) > 1e-5 for k in keys):
        raise ValueError("Plate keys differ from validated native PCB switch stems")
    folder = LOCAL/str(count)
    folder.mkdir(parents=True,exist_ok=True)
    doc = App.newDocument(f'KeycultPlate{count}')
    doc.addObject('App::FeaturePython','Study').addProperty('App::PropertyString','Status').Status = 'NOT FOR FABRICATION; all proposed dimensions require review'
    parameters = doc.getObject('Study')
    for name,value in STUDY.items():
        parameters.addProperty('App::PropertyFloat',name,'Assumptions')
        setattr(parameters,name,value)
    parameters.addProperty('App::PropertyString','EditInstructions').EditInstructions = 'Regenerate with scripts/build_plate.py after changing STUDY. Display properties do not drive geometry.'
    original = feature(doc,'OfficialNoLeafReference',donor,'Pinned Keycult source; personal local use only')
    base = feature(doc,'BlankWithOriginalPerimeter',blank,'Exact outer wire; removes only original key/stabilizer openings')
    width = s['switch_opening']
    switch_faces = [rect(k['x_mm']-width/2,k['y_mm']-width/2,width,width) for k in keys]
    cuts = [f.extrude(V(0,0,-t)) for f in switch_faces]
    holes = [(x,y) for x in (-s['mount_pitch']/2,s['mount_pitch']/2) for y in (-s['mount_pitch']/2,s['mount_pitch']/2)]
    opening = Part.makeCylinder(radius,t,V(0,0,-t))
    mount_cuts = [Part.makeCylinder(s['mount_hole_diameter']/2,t,V(x,y,-t)) for x,y in holes]
    tool = feature(doc,'ProposedPlateCutters',Part.makeCompound(cuts+[opening]+mount_cuts),'Square switch openings and central window; nominal dimensions recorded in Study')
    plate = cut(doc,'Plate',base,tool)
    carrier = rect(-half,-half,s['carrier_width'],s['carrier_width'])
    if count == 62:
        carrier = carrier.fuse(rect(-s['rear_shelf_width']/2,s['rear_extension_y'],s['rear_shelf_width'],-s['rear_extension_y'])).removeSplitter()
    carrier = carrier.cut(Part.Face(Part.Wire([Part.makeCircle(radius)])))
    for x,y in holes:
        carrier = carrier.cut(Part.Face(Part.Wire([Part.makeCircle(s['mount_hole_diameter']/2,V(x,y,0))])))
    carrier = carrier.removeSplitter()
    adapter = feature(doc,'CarrierConcept',carrier.extrude(V(0,0,s['carrier_thickness'])),'Own mounting concept: M2 clearance holes; no verified holder or sensor interface. Rear shelf only on Keycult62.')
    board = pcb_shape(report, s['pcb_top_z'], s['pcb_thickness'])
    board_obj = feature(doc,'CurrentPCBEnvelope',board,'Validated native XY envelope and opening; top Z and thickness assumed in Study')
    ball = Part.makeSphere(s['ball_radius'],V(0,0,s['ball_center_z']))
    ball_obj = feature(doc,'BallAtIllustrativeZ8',ball,'Illustration, not the supported position in an actual holder')
    raised = feature(doc,'BallAtAlternativeZ13',Part.makeSphere(s['ball_radius'],V(0,0,s['alternative_ball_center_z'])),'Alternative height only; holder clearance unverified')
    proposal = feature(doc,'UnappliedPCBOpeningTool',Part.makeCylinder(radius,s['pcb_thickness'],V(0,0,s['pcb_top_z']-s['pcb_thickness'])),'Circular study tool, distinct from any native rounded opening; not an approved holder opening')
    caps = feature(doc,'KeycapPlanProxies',Part.makeCompound([rect(k['x_mm']-cap_width(k)/2,k['y_mm']-9,cap_width(k),18,5).extrude(V(0,0,5)) for k in keys]),'Width 18+(width_u-1)*19.05mm, height 18mm; Z=5..10 illustrative, no switch travel or real keycap height model')
    pcb_clash = ball.common(board).Volume
    expected_clash = pcb_clash > 0
    feature(doc,'BallPCBInterference',ball.common(board),'Calculated intersection with current native PCB at the illustrative ball height')
    for obj in (plate,adapter):
        if not obj.Shape.isValid() or len(obj.Shape.Solids)!=1:
            raise ValueError(f'{count}: {obj.Name} must remain one valid solid')
    if plate.Shape.common(donor).Volume <= 0 or blank.cut(plate.Shape.fuse(tool.Shape)).Volume > 1e-5:
        raise ValueError('Unexpected blank/cut topology')
    plate_face = max((f for f in plate.Shape.Faces if abs(f.CenterOfMass.z)<1e-6 and abs(f.normalAt(0,0).z)>.99),key=lambda f:f.Area)
    restored_face = Part.Face(plate_face.OuterWire)
    perimeter_error = restored_face.cut(face).Area + face.cut(restored_face).Area
    if perimeter_error > 1e-5 or len(plate_face.Wires)!=1+key_count+5:
        raise ValueError(f'Outer contour or hole count differs: area={perimeter_error}, wires={len(plate_face.Wires)}, expected={1+key_count+5}')
    for k,cutter in zip(keys,cuts):
        if cutter.cut(blank).Volume > 1e-6:
            raise ValueError('Switch opening outside donor perimeter: '+k['id'])
    if any(carrier.common(f).Area > 1e-6 for f in switch_faces):
        raise ValueError('Carrier intersects switch aperture')
    clearance = min(carrier.distToShape(rect(k['x_mm']-cap_width(k)/2,k['y_mm']-9,cap_width(k),18))[0] for k in keys)
    doc.recompute()
    doc.saveAs(str(folder/f'plate-{count}.FCStd'))
    Part.export([plate],str(folder/f'plate-{count}.step'))
    Part.export([adapter],str(folder/f'carrier-{count}.step'))
    reread = Part.read(str(folder/f'plate-{count}.step'))
    if not reread.isValid() or abs(reread.Volume-plate.Shape.Volume)>1e-4:
        raise ValueError('STEP roundtrip mismatch')
    write_dxf(plate_face,folder/f'plate-{count}-DRAFT.dxf')
    svg(plate_face,carrier,keys,key_count,folder/'plan.svg',report.get('pcb_opening'))
    measurements = dict(keys=key_count,plate_valid=True,plate_solids=len(plate.Shape.Solids),
        exact_outer_wire_preserved_area_error_mm2=perimeter_error,switch_coordinate_max_error_mm=max(math.dist(actual[k['id']],[k['x_mm'],k['y_mm']]) for k in keys),
        switch_openings=key_count,central_openings=1,carrier_mount_holes=4,
        carrier_to_keycap_plan_clearance_mm=clearance,
        plate_to_switch_opening_min_edge_mm=min(face.OuterWire.distToShape(c.Wires[0])[0] for c in cuts),
        ball_pcb_interference_mm3=pcb_clash,expected_ball_pcb_conflict=expected_clash,
        ball_plate_interference_mm3=ball.common(plate.Shape).Volume,
        alternative_z13_ball_to_pcb_gap_mm=raised.Shape.distToShape(board)[0],
        pcb_native_sha256=sha(pcb),pcb_modified=False,pcb_opening=report.get('pcb_opening'),
        step_roundtrip_volume_error_mm3=abs(reread.Volume-plate.Shape.Volume))
    (folder/'checks.json').write_text(json.dumps(measurements,indent=2)+'\n')
    App.closeDocument(doc.Name)
    reopened = App.openDocument(str(folder/f'plate-{count}.FCStd'))
    if not reopened.getObject('Plate').Shape.isValid():
        raise ValueError('FCStd readback invalid')
    App.closeDocument(reopened.Name)
    return measurements


def build_printable_prototype(layout, face, blank):
    from build_printable_holder import S, geometry, check, valid, rounded_box

    folder = LOCAL.parent / 'prototype-62'
    folder.mkdir(parents=True, exist_ok=True)
    keys = layout['rear_clearance']['keys']
    pcb, pcb_validation = pcb_report(62, layout['rear_clearance'])
    coordinates_path = pcb.parent/'reports/switch-coordinates.json'
    coordinates = json.loads(coordinates_path.read_text())
    actual = {k['id']: [k['actual_stem_mm'][0]-180, k['actual_stem_mm'][1]-100] for k in coordinates}
    assert set(actual) == {k['id'] for k in keys}, 'Prototype and native PCB switch IDs differ'
    stem_error = max(math.dist(actual[k['id']], [k['x_mm'], k['y_mm']]) for k in keys)
    assert stem_error < 1e-5, 'Prototype openings differ from native PCB switch stems'
    holder_report = json.loads((ROOT/'design/printable-holder/checks.json').read_text())
    layout_path = ROOT/'design/preliminary/layout.json'
    if holder_report['pcb_sha256'] != sha(pcb) or holder_report['layout_sha256'] != sha(layout_path) or holder_report['settings_mm'] != S:
        raise ValueError('Printable holder inputs are stale; rebuild the holder first')
    parts, references = geometry()
    verified = check(parts, references)
    t = S['plate_thickness']
    cutters = [rect(k['x_mm']-7, k['y_mm']-7, 14, 14).extrude(V(0, 0, -t)) for k in keys]
    cutters += [rounded_box((-18, -18, 18, 18), 3, -t, t),
                rounded_box((-28, -47, 28, -28), 1, -t, t)]
    cutters += [Part.makeCylinder(1.15, t, V(x, y, -t)) for x in (-21, 21) for y in (-21, 21)]
    plate = blank.cut(Part.makeCompound(cutters)).removeSplitter()
    valid(plate, 'Current 62-key prototype plate')
    top = max((f for f in plate.Faces if abs(f.CenterOfMass.z) < 1e-6 and f.normalAt(0, 0).z > .99), key=lambda f: f.Area)
    restored = Part.Face(top.OuterWire)
    outline_error = restored.cut(face).Area+face.cut(restored).Area
    assert outline_error < 1e-5 and len(top.Wires) == 69, 'Prototype perimeter or aperture count changed'
    assert all(c.cut(blank).Volume < 1e-6 for c in cutters), 'Prototype cutter outside official perimeter'
    mounted = {n: p for n, p in parts.items() if n != 'SeatCoupon'}
    plate_overlaps = {n: p.common(plate).Volume for n, p in (mounted | references).items() if n != 'CandidatePlate'}
    assert all(v < 1e-6 for v in plate_overlaps.values()), plate_overlaps
    doc = App.newDocument('Current62PrototypeAssembly')
    metadata = doc.addObject('App::FeaturePython', 'PrototypeAssumptions')
    metadata.addProperty('App::PropertyString', 'Warning').Warning = ('LOCAL PERSONAL STUDY. Official historical perimeter, not registered to the owned case. '
        'Sensor board, component bodies, switch/keycap Z, material and tolerances unmeasured. Not a manufacturing or case-fit approval.')
    metadata.addProperty('App::PropertyString', 'ParametersJSON').ParametersJSON = json.dumps(S)
    metadata.addProperty('App::PropertyString', 'PCB_SHA256').PCB_SHA256 = sha(pcb)
    plate_obj = feature(doc, 'Plate', plate, 'Exact pinned NoLeaf outer wire; 62 square 14mm switches, 36x36 R3 window, four D2.3 holes at +/-21mm, rear electronics access x+/-28 y-47..-28 R1; nominal aluminum thickness 1.5mm')
    objects = [plate_obj]
    for name, shape in (mounted | references).items():
        if name == 'CandidatePlate':
            continue
        obj = feature(doc, name, shape, 'Independent printable part' if name in mounted else 'Nominal purchase-part or native PCB envelope reference; not actual component CAD')
        objects.append(obj)
    for name, shapes in (
        ('KeycapProxies', [rect(k['x_mm']-cap_width(k)/2, k['y_mm']-9, cap_width(k), 18, 5).extrude(V(0, 0, 10)) for k in keys]),
        ('SwitchProxies', [rect(k['x_mm']-7.8, k['y_mm']-7.8, 15.6, 15.6, -9).extrude(V(0, 0, 14)) for k in keys])):
        feature(doc, name, Part.makeCompound(shapes), 'Unmeasured rectangular envelope at current XY key stems; not component models')
    model_checks, omitted_models, native_model_check = [], [], None
    component_shapes = {}
    model_manifest = LOCAL.parent/'pcb-62/component-models.json'
    if model_manifest.exists():
        models = json.loads(model_manifest.read_text())
        assert models['source_pcb_sha256'] == sha(pcb), 'Component model placements are stale'
        native = None
        if models.get('native_step'):
            source = models['native_step']
            assert source['source_pcb_sha256'] == sha(pcb) and sha(Path(source['path'])) == source['sha256'], 'Native six-component STEP is stale'
            native = Part.read(source['path'])
            assert native.isValid() and len(native.Solids) == 7, 'Expected bare PCB and six component solids'
            transform = App.Matrix()
            transform.A22, transform.A14, transform.A24 = -1, -180, -100
            # KiCad's STEP exporter offsets component models 0.0012mm above the nominal mounting plane.
            transform.A34 = S['pcb_top_z']-S['pcb_thickness']-.0012
            native = native.transformGeometry(transform)
            native_model_check = dict(sha256=source['sha256'], model_origin_epsilon_normalized_mm=.0012)
        for component in models['parts']:
            # shortcut: this stock switch STEP exits FreeCAD's loader; retry after the upstream/importer defect is resolved.
            if component['footprint'] == 'SW_SPST_TL3342':
                omitted_models.append(component['ref']+': stock STEP loader exits interpreter with status 1')
                continue
            assert component['side'] == 'top', 'Shallow prototype components must be on the top side'
            for model in component['models']:
                path = Path(model['local_step'])
                assert sha(path) == model['sha256'], 'Stock component STEP hash changed'
                assert model['offset_xyz'] == [0., 0., 0.] and model['rotation_xyz'] == [0., 0., 0.] and model['scale_xyz'] == [1., 1., 1.], 'Unsupported model transform'
                shape = Part.read(str(path))
                assert shape.isValid() and shape.Volume > 0, 'Invalid stock component STEP'
                angle = math.radians(component['pcb_rotation_deg'])
                matrix = App.Matrix()
                matrix.A11, matrix.A12 = math.cos(angle), -math.sin(angle)
                matrix.A21, matrix.A22 = -math.sin(angle), -math.cos(angle)
                matrix.A14, matrix.A24 = component['pcb_origin_xy_mm'][0]-180, component['pcb_origin_xy_mm'][1]-100
                matrix.A34 = S['pcb_top_z']
                placed = shape.transformGeometry(matrix)
                assert placed.isValid(), 'Invalid transformed component STEP'
                native_difference = placed.cut(native).Volume if native is not None else None
                assert native_difference is None or native_difference < 1e-5, (component['ref'], 'Standalone model transform differs from native KiCad STEP', native_difference)
                overlap = {n: placed.common(p).Volume for n, p in dict(Plate=plate,
                    Switches=doc.getObject('SwitchProxies').Shape, Keycaps=doc.getObject('KeycapProxies').Shape, **mounted, **component_shapes).items()}
                assert all(v < 1e-6 for v in overlap.values()), (component['ref'], overlap)
                component_shapes['PCB_'+component['ref']] = placed
                b = placed.BoundBox
                model_checks.append(dict(ref=component['ref'], source_sha256=model['sha256'],
                    bounds_mm=[b.XMin, b.YMin, b.ZMin, b.XMax, b.YMax, b.ZMax],
                    switch_proxy_minimum_gap_mm=placed.distToShape(doc.getObject('SwitchProxies').Shape)[0],
                    native_missing_volume_mm3=native_difference, overlaps_mm3=overlap))
                objects.append(feature(doc, 'PCB_'+component['ref'], placed, 'Pinned stock component STEP at native footprint origin; unmeasured supplied component; model Y inverted to layout Y'))
    else:
        omitted_models.append('Native component placement manifest unavailable')
    doc.recompute()
    fcstd, step = folder/'prototype-62.FCStd', folder/'prototype-62.step'
    doc.saveAs(str(fcstd))
    Part.export(objects, str(step))
    reread = Part.read(str(step))
    assert reread.isValid() and len(reread.Solids) == sum(len(o.Shape.Solids) for o in objects)
    expected_volume = sum(o.Shape.Volume for o in objects)
    assert math.isclose(reread.Volume, expected_volume, rel_tol=1e-6, abs_tol=1e-3)
    Part.export([plate_obj], str(folder/'plate-62-PROTOTYPE.step'))
    write_dxf(top, folder/'plate-62-PROTOTYPE.dxf')
    paths = ' '.join(wire_path(w) for w in top.Wires)
    (folder/'plan.svg').write_text('<svg xmlns="http://www.w3.org/2000/svg" width="330mm" height="150mm" viewBox="-165 -75 330 150">'
        '<rect x="-165" y="-75" width="330" height="150" fill="white"/>'
        '<text x="-155" y="-63" font-family="sans-serif" font-size="4">62-key shallow prototype / 36x36 R3 / 4x D2.3 on 42 square / LOCAL PERSONAL STUDY</text>'
        f'<path d="{paths}" fill="#d5dce3" fill-rule="evenodd" stroke="#334155" stroke-width=".2"/>'
        '<circle cx="0" cy="0" r="17" fill="#fdba74" fill-opacity=".7"/>'
        '<text x="-155" y="67" font-family="sans-serif" font-size="3">Nominal 1.5mm aluminum. Owned case revision, switch clip fit and stiffness unverified.</text></svg>')
    App.closeDocument(doc.Name)
    reopened = App.openDocument(str(fcstd))
    valid(reopened.getObject('Plate').Shape, 'Current assembly FCStd plate')
    for name in mounted:
        valid(reopened.getObject(name).Shape, 'Current assembly FCStd '+name)
    App.closeDocument(reopened.Name)
    assert sha(pcb) == verified['pcb_sha256'] and sha(layout_path) == verified['layout_sha256'], 'PCB or layout changed during assembly generation'
    result = dict(status='digital_prototype_nominal_geometry_checked_not_case_or_hardware_qualified',
        official_outer_wire_area_error_mm2=outline_error, source_step_sha256=STEP_SHA,
        dxf_curve_deflection_mm=.01,
        pcb_sha256=sha(pcb), pcb_opening=pcb_validation['pcb_opening'], layout_sha256=verified['layout_sha256'],
        switch_coordinates_sha256=sha(coordinates_path), switch_coordinate_max_error_mm=stem_error,
        switch_openings=62, switch_opening_mm=14., plate_thickness_mm=t, window_bounds_mm=[-18., -18., 18., 18.], window_corner_radius_mm=3.,
        rear_electronics_relief_bounds_mm=[-28., -47., 28., -28.], rear_electronics_relief_corner_radius_mm=1.,
        mount_pitch_mm=42., mount_hole_diameter_mm=2.3, max_plate_overlap_mm3=max(plate_overlaps.values()),
        step_volume_relative_error=abs(reread.Volume-expected_volume)/expected_volume,
        holder_checks=verified, stock_component_models=model_checks, omitted_component_models=omitted_models,
        native_six_component_step_check=native_model_check,
        omitted='Other PCB component bodies, actual supplied component dimensions, case, real sensor lens and connector, and switch/keycap profiles')
    (folder/'checks.json').write_text(json.dumps(result, indent=2)+'\n')
    return result


def main():
    layout = json.loads((ROOT/'design/preliminary/layout.json').read_text())
    face,blank,_ = source_blank()
    prototype = build_printable_prototype(layout, face, blank)
    print(json.dumps(dict(output=str(LOCAL.parent/'prototype-62'), freecad=App.Version(), checks=prototype), indent=2))


if __name__ == '__main__':
    main()
