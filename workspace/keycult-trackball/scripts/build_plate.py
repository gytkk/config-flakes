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


def svg(face, carrier_face, keys, count, path):
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
<text x="-155" y="78">PCB is still uncut. Ball center Z={s['ball_center_z']:g}; PCB top Z={s['pcb_top_z']:g} assumed. See interference report.</text></g></svg>''')


def build(count, layout, face, blank, donor):
    s = STUDY
    t, half, radius = s['plate_thickness'], s['carrier_width']/2, s['opening_diameter']/2
    variant = 'preferred' if count == 65 else 'rear_clearance'
    keys = layout[variant]['keys']
    key_count = len(keys)
    directory = ROOT / 'design/pcb' / ('62' if count == 62 else '')
    report = json.loads((directory/'reports/validation.json').read_text())
    pcb = directory/f'keycult{count}.kicad_pcb'
    if sha(pcb) != report['artifact_sha256'][pcb.name]:
        raise ValueError("PCB has changed since native coordinate validation; rerun validate_pcb.py")
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
    xmin,ymin,xmax,ymax = report['envelope_bounds_mm']
    board = Part.makeBox(xmax-xmin,ymax-ymin,s['pcb_thickness'],V(xmin-180,ymin-100,s['pcb_top_z']-s['pcb_thickness']))
    board_obj = feature(doc,'CurrentPCBEnvelope',board,'Actual XY envelope; top Z assumed in Study; no central cutout in native PCB')
    ball = Part.makeSphere(s['ball_radius'],V(0,0,s['ball_center_z']))
    ball_obj = feature(doc,'BallAtIllustrativeZ8',ball,'Illustration, not the supported position in an actual holder')
    raised = feature(doc,'BallAtAlternativeZ13',Part.makeSphere(s['ball_radius'],V(0,0,s['alternative_ball_center_z'])),'Alternative height only; holder clearance unverified')
    proposal = feature(doc,'UnappliedPCBOpeningTool',Part.makeCylinder(radius,s['pcb_thickness'],V(0,0,s['pcb_top_z']-s['pcb_thickness'])),'Proposed mechanical opening, NOT applied to KiCad and NOT approved holder opening')
    caps = feature(doc,'KeycapPlanProxies',Part.makeCompound([rect(k['x_mm']-cap_width(k)/2,k['y_mm']-9,cap_width(k),18,5).extrude(V(0,0,5)) for k in keys]),'Width 18+(width_u-1)*19.05mm, height 18mm; Z=5..10 illustrative, no switch travel or real keycap height model')
    pcb_clash = ball.common(board).Volume
    # A clash is the expected finding of this scenario, not a pass hidden by removal.
    expected_clash = pcb_clash > 0
    feature(doc,'BallPCBInterference',ball.common(board),'Known conflict in current uncut PCB at the illustrative ball height')
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
    svg(plate_face,carrier,keys,key_count,folder/'plan.svg')
    measurements = dict(keys=key_count,plate_valid=True,plate_solids=len(plate.Shape.Solids),
        exact_outer_wire_preserved_area_error_mm2=perimeter_error,switch_coordinate_max_error_mm=max(math.dist(actual[k['id']],[k['x_mm'],k['y_mm']]) for k in keys),
        switch_openings=key_count,central_openings=1,carrier_mount_holes=4,
        carrier_to_keycap_plan_clearance_mm=clearance,
        plate_to_switch_opening_min_edge_mm=min(face.OuterWire.distToShape(c.Wires[0])[0] for c in cuts),
        ball_pcb_interference_mm3=pcb_clash,expected_ball_pcb_conflict=expected_clash,
        ball_plate_interference_mm3=ball.common(plate.Shape).Volume,
        alternative_z13_ball_to_pcb_gap_mm=raised.Shape.distToShape(board)[0],
        pcb_native_sha256=sha(pcb),pcb_modified=False,
        step_roundtrip_volume_error_mm3=abs(reread.Volume-plate.Shape.Volume))
    (folder/'checks.json').write_text(json.dumps(measurements,indent=2)+'\n')
    App.closeDocument(doc.Name)
    reopened = App.openDocument(str(folder/f'plate-{count}.FCStd'))
    if not reopened.getObject('Plate').Shape.isValid():
        raise ValueError('FCStd readback invalid')
    App.closeDocument(reopened.Name)
    return measurements


def main():
    LOCAL.mkdir(parents=True,exist_ok=True)
    layout = json.loads((ROOT/'design/preliminary/layout.json').read_text())
    face,blank,donor = source_blank()
    results = [build(count,layout,face,blank,donor) for count in (65,62)]
    report = dict(status='mechanical study only; known PCB interference, no fit or manufacturing approval',
                  freecad=App.Version(),parameters=STUDY,source_step_sha256=STEP_SHA,
                  transform='x=STEP.x-142.875; y=STEP.z-47.625; z=STEP.y. PCB x-180,y-100. Physical datum unverified.',
                  dxf_curve_deflection_mm=.01,results=results,
                  unresolved=['Case revision and registration','Switch clip opening/tolerance and plate material',
                              'Actual holder/sensor CAD and fastening interface','PCB opening or acceptable raised assembly',
                              'Floor clearance, screws, component heights, wiring and assembly access','Plate/carrier stiffness and optical alignment under load'])
    (LOCAL/'review.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))


if __name__ == '__main__':
    main()
