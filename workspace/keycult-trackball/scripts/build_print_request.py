# /// script
# requires-python = ">=3.11"
# ///
"""Build independent bench fixtures and a self-contained PLA print request."""
import csv
import hashlib
import itertools
import json
import math
from pathlib import Path
import zipfile

from build_printable_holder import (
    App, Mesh, MeshPart, Part, ROOT, S, V, box, check, cylinder, geometry,
    rounded_box, valid,
)

OUT = ROOT / 'design/print-request'
HOLDER = ROOT / 'design/printable-holder'
KEY_IDS = ('r1c5', 'r1c9', 'r2c5', 'r2c9', 'r3c5', 'r3c9',
           'r4c5', 'r4c6', 'r4c8', 'r4c9')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fixtures(layout):
    keys = [k for k in layout['keys'] if k['id'] in KEY_IDS]
    assert len(keys) == len(KEY_IDS)
    # This rectangle is a tabletop jig, not the Keycult plate outline.
    plate = box(-55, -32, -S['plate_thickness'], 110, 87, S['plate_thickness'])
    plate = plate.cut(rounded_box((-18, -18, 18, 18), 3, -2, 3))
    for x, y in itertools.product((-S['mount_pitch']/2, S['mount_pitch']/2), repeat=2):
        plate = plate.cut(cylinder(S['clearance_diameter']/2, -2, 3, x, y))
    for k in keys:
        plate = plate.cut(box(k['x_mm']-7, k['y_mm']-7, -2, 14, 14, 3))
    # Open-bottom frame supports the plate perimeter; a loose rim locates it.
    stand = box(-57.3, -34.3, -20, 114.6, 91.6, 20)
    stand = stand.cut(box(-48, -29, -21, 96, 81, 23))
    stand = stand.cut(box(-55.3, -32.3, -S['plate_thickness'], 110.6, 87.6, 3))
    hole = layout['pcb_opening']
    dummy = box(-29, -29, S['pcb_top_z']-S['pcb_thickness'], 58, 58, S['pcb_thickness'])
    dummy = dummy.cut(rounded_box(hole['bounds_mm'], hole['corner_radius_mm'], -8, 5))
    parts = dict(BenchPlate=plate.removeSplitter(), BenchStand=stand.removeSplitter(),
                 PCBOpeningGauge=dummy.removeSplitter())
    for height in (3.75, 3.0):
        name = 'SensorSpacer'+str(height).replace('.', 'p')+'mm'
        parts[name] = cylinder(2.2, 0, height).cut(cylinder(1.15, -1, height+2))
    return parts, keys


def check_fixtures(parts, holder, refs, keys, opening):
    for name, shape in parts.items():
        valid(shape, name)
    plate, stand, dummy = (parts[n] for n in ('BenchPlate', 'BenchStand', 'PCBOpeningGauge'))
    mounted = {n: s for n, s in holder.items() if n != 'SeatCoupon'}
    hardware = {n: s for n, s in refs.items() if n not in ('CandidatePlate', 'CurrentPCB')}
    overlaps = {}
    for name, fixture in (('plate', plate), ('stand', stand), ('dummy', dummy)):
        for other, shape in (mounted | hardware).items():
            overlaps[name+'/'+other] = fixture.common(shape).Volume
    for a, b in itertools.combinations(('BenchPlate', 'BenchStand', 'PCBOpeningGauge'), 2):
        overlaps[a+'/'+b] = parts[a].common(parts[b]).Volume
    for k in keys:
        x, y = k['x_mm'], k['y_mm']
        switch_opening = box(x-7, y-7, -2, 14, 14, 3)
        assert plate.common(switch_opening).Volume < 1e-6, ('Switch opening blocked', k['id'])
        # Use the existing conservative body envelope below the plate only.
        body = box(x-7.8, y-7.8, -9, 15.6, 15.6, 7.5)
        width = 18+(k['width_u']-1)*19.05
        cap = box(x-width/2, y-9, 5, width, 18, 10)
        for name, shape in dict(stand=stand, dummy=dummy, **mounted, **hardware).items():
            overlaps[k['id']+'/body/'+name] = body.common(shape).Volume
            overlaps[k['id']+'/cap/'+name] = cap.common(shape).Volume
    assert max(overlaps.values()) < 1e-6, {k: v for k, v in overlaps.items() if v >= 1e-6}
    # The plate must sit on a ledge, not just float inside the locating rim.
    lowered = plate.copy()
    lowered.translate(V(0, 0, -.1))
    assert lowered.common(stand).Volume > 1, 'Missing plate support'
    for offset in (0, 1, 5, 20):
        lifted = plate.copy()
        lifted.translate(V(0, 0, offset))
        assert lifted.common(stand).Volume < 1e-6, 'Plate lift-out blocked'
    for offset in (-20, -10, -3, 0):
        moving = dummy.copy()
        moving.translate(V(0, 0, offset))
        for name, shape in (mounted | hardware | dict(plate=plate, stand=stand)).items():
            assert moving.common(shape).Volume < 1e-6, ('Gauge insertion blocked', offset, name)
    for name, height in (('SensorSpacer3p75mm', 3.75), ('SensorSpacer3p0mm', 3.0)):
        shape = parts[name]
        expected = math.pi*(2.2**2-1.15**2)*height
        assert math.isclose(shape.Volume, expected, rel_tol=1e-8)
        assert math.isclose(shape.BoundBox.ZLength, height, abs_tol=1e-8)
    return dict(max_overlap_mm3=max(overlaps.values()), switch_centers=keys,
                plate_size_mm=[110, 87, S['plate_thickness']], stand_size_mm=[114.6, 91.6, 20],
                plate_rim_clearance_per_side_mm=.3, tabletop_to_plate_top_mm=20,
                gauge_size_mm=[58, 58, S['pcb_thickness']],
                gauge_opening=opening,
                gauge_position='Hand-held only; no Z registration or attachment',
                plate_removal_offsets_checked_mm=[0, 1, 5, 20],
                gauge_insertion_offsets_checked_mm=[-20, -10, -3, 0])


def mesh_check(path, shape):
    mesh = Mesh.Mesh(str(path))
    assert mesh.isSolid() and mesh.countComponents() == 1, (path.name, 'Mesh not closed/connected')
    error = abs(abs(mesh.Volume)-shape.Volume)/shape.Volume
    assert error < .005, (path.name, 'Mesh volume drift', error)
    bb = mesh.BoundBox
    for actual, expected in zip((bb.XLength, bb.YLength, bb.ZLength),
                                (shape.BoundBox.XLength, shape.BoundBox.YLength, shape.BoundBox.ZLength)):
        assert abs(actual-expected) < .1, (path.name, 'Mesh bounds drift', actual, expected)
    assert abs(bb.ZMin) < 1e-5, (path.name, 'STL not on Z=0')
    return dict(dimensions_mm=[round(bb.XLength, 3), round(bb.YLength, 3), round(bb.ZLength, 3)],
                mesh_volume_relative_error=error, sha256=sha(path))


def save_cad(parts, holder, refs):
    doc = App.newDocument('PrintRequestBench')
    shapes = {n: s for n, s in (holder | refs).items()
              if n not in ('SeatCoupon', 'CandidatePlate', 'CurrentPCB')}
    shapes.update({n: parts[n] for n in ('BenchPlate', 'BenchStand', 'PCBOpeningGauge')})
    object_names = {}
    for name, shape in shapes.items():
        obj = doc.addObject('Part::Feature', name)
        obj.Label = name
        obj.Shape = shape
        obj.Visibility = name != 'PCBOpeningGauge'
        object_names[name] = obj.Name
    doc.recompute()
    path = OUT/'bench-assembly.FCStd'
    doc.saveAs(str(path))
    App.closeDocument(doc.Name)
    reopened = App.openDocument(str(path))
    for name, shape in shapes.items():
        saved = reopened.getObject(object_names[name]).Shape
        valid(saved, name+' readback')
        assert math.isclose(saved.Volume, shape.Volume, abs_tol=1e-6)
    App.closeDocument(reopened.Name)
    return path


def plan_svg(keys):
    elements = ['<rect x="-55" y="-32" width="110" height="87" fill="#dce6ed" stroke="#334155"/>',
                '<rect x="-18" y="-18" width="36" height="36" rx="3" fill="white" stroke="#334155"/>',
                '<circle cx="0" cy="0" r="17" fill="#fdba74" fill-opacity=".7"/>']
    for x, y in itertools.product((-21, 21), repeat=2):
        elements.append(f'<circle cx="{x}" cy="{y}" r="1.15" fill="white" stroke="#334155"/>')
    for k in keys:
        x, y = k['x_mm'], k['y_mm']
        elements += [f'<rect x="{x-7}" y="{y-7}" width="14" height="14" fill="white" stroke="#334155"/>',
                     f'<text x="{x}" y="{y+1.3}" text-anchor="middle" font-size="4">{k["label"]}</text>']
    (OUT/'fixture-plan.svg').write_text('<svg xmlns="http://www.w3.org/2000/svg" width="780" height="690" viewBox="0 0 130 115">'
        '<rect width="130" height="115" fill="white"/>'
        '<g font-family="sans-serif" fill="#172033" stroke-width=".25">'
        '<text x="5" y="7" font-size="4">PLA bench jig: 110 x 87 mm / 10 switches</text>'
        '<text x="5" y="13" font-size="3">Rear: T / Y. Front: Del / M1 / M2 / Bksp.</text>'
        '<g transform="translate(65,51)">'+''.join(elements)+'</g>'
        '<text x="5" y="111" font-size="3">Independent tabletop fixture. NOT a Keycult replacement plate.</text></g></svg>')


def assembly_preview(parts, holder, refs, keys):
    shapes = {n: (s, '#238b83') for n, s in holder.items() if n != 'SeatCoupon'}
    shapes.update({n: (s, '#bca56b') for n, s in refs.items()
                   if n not in ('CandidatePlate', 'CurrentPCB', 'SensorBoardAssumption')})
    shapes['Ball'] = (refs['Ball'], '#f6ac63')
    shapes['BenchPlate'] = (parts['BenchPlate'], '#aabacb')
    shapes['BenchStand'] = (parts['BenchStand'], '#596b80')
    for k in keys:
        width = 18+(k['width_u']-1)*19.05
        shapes[k['id']] = (box(k['x_mm']-width/2, k['y_mm']-9, 5, width, 18, 10), '#eee8db')
    faces = []
    view = V(1, 1.4, 1.1)
    for shape, color in shapes.values():
        mesh = MeshPart.meshFromShape(Shape=shape, LinearDeflection=.3, AngularDeflection=.35, Relative=False)
        vertices, triangles = mesh.Topology
        for triangle in triangles:
            points = [vertices[i] for i in triangle]
            normal = (points[1]-points[0]).cross(points[2]-points[0])
            if normal.dot(view) <= 0:
                continue
            coords = [(p.x*.814-p.y*.581, p.x*.31+p.y*.435-p.z*.846) for p in points]
            depth = sum(p.dot(view) for p in points)/3
            shade = .65+.35*abs(normal.normalize().dot(V(.3, .4, .866)))
            rgb = [int(int(color[i:i+2], 16)*shade) for i in (1, 3, 5)]
            paint = f'rgb({rgb[0]},{rgb[1]},{rgb[2]})'
            faces.append((depth, f'<polygon points="{" ".join(f"{x:.3f},{y:.3f}" for x,y in coords)}" '
                                f'fill="{paint}" stroke="{paint}" stroke-width=".04"/>'))
    (OUT/'bench-preview.svg').write_text('<svg xmlns="http://www.w3.org/2000/svg" width="900" height="720" viewBox="0 0 900 720">'
        '<rect width="900" height="720" fill="#f6f5f1"/>'
        '<g transform="translate(450,340) scale(4.8)">'+''.join(f for _, f in sorted(faces))+'</g>'
        '<g font-family="sans-serif" fill="#172033"><text x="30" y="40" font-size="24">PLA bench assembly / 10-key position study</text>'
        '<text x="30" y="655" font-size="17">Print the holder, plate and stand. Use real ball, bearings, switches and keycaps.</text>'
        '<text x="30" y="686" font-size="17">Horizontal bench fixture only. Case fit, stiffness and physical assembly unverified.</text></g></svg>')


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    holder, refs = geometry()
    holder_checks = check(holder, refs)
    layout_path = ROOT/'design/preliminary/layout.json'
    layout = json.loads(layout_path.read_text())['rear_clearance']
    parts, keys = fixtures(layout)
    checks = check_fixtures(parts, holder, refs, keys, layout['pcb_opening'])
    quantities = dict(Cup=1, SensorCarriage=1, SensorCap=2, SeatCoupon=1,
                      SensorSpacer1=2, SensorSpacer2=2, SensorSpacer3=2, SensorSpacer4=2,
                      BenchPlate=1, BenchStand=1, PCBOpeningGauge=1,
                      SensorSpacer3p75mm=4, SensorSpacer3p0mm=4)
    records, files = [], {}
    source_hashes = json.loads((HOLDER/'checks.json').read_text())['artifact_sha256']
    for name, shape in (holder | parts).items():
        filename = name+'-PROTOTYPE.stl'
        if name in holder:
            path = HOLDER/filename
            assert sha(path) == source_hashes[filename], ('Holder artifact changed', filename)
        else:
            path = OUT/filename
            printable = shape.copy()
            bb = printable.BoundBox
            printable.translate(V(-bb.XMin, -bb.YMin, -bb.ZMin))
            mesh = MeshPart.meshFromShape(Shape=printable, LinearDeflection=.05, AngularDeflection=.15, Relative=False)
            mesh.write(str(path))
        record = dict(file='stl/'+filename, quantity=quantities[name], **mesh_check(path, shape))
        records.append(record)
        files[record['file']] = path
    assert sum(r['quantity'] for r in records) == 24 and len(records) == 13
    save_cad(parts, holder, refs)
    plan_svg(keys)
    assembly_preview(parts, holder, refs, keys)
    with (OUT/'print-list.csv').open('w', newline='', encoding='utf-8-sig') as handle:
        writer = csv.writer(handle, lineterminator='\n')
        writer.writerow(['file', 'quantity_including_spares', 'X_mm', 'Y_mm', 'Z_mm'])
        for r in records:
            writer.writerow([r['file'], r['quantity'], *r['dimensions_mm']])
    (OUT/'PRINT-LIST.txt').write_text('출력 수량표: STL 13종, 예비분 포함 총 24개\n단위 mm, 배율 100%. 파일명 뒤 숫자는 수량이 아닙니다.\n\n'+
        '\n'.join(f'{r["quantity"]}개 | {r["file"]} | {" x ".join(map(str, r["dimensions_mm"]))} mm' for r in records)+'\n', encoding='utf-8')
    report = dict(status='nominal_geometry_and_archive_checked_not_print_or_hardware_qualified',
                  input_sha256={str(p.relative_to(ROOT)): sha(p) for p in
                                (Path(__file__), ROOT/'scripts/build_printable_holder.py', layout_path, HOLDER/'checks.json')},
                  holder_status=holder_checks['status'], sensor_travel_hardware=holder_checks['sensor_travel_hardware'],
                  fixture_checks=checks, printable_files=records,
                  limitations=['No physical print, slicer/toolpath or strength test',
                               'No real sensor/lens/cable/switch-clip geometry or case fit qualification',
                               'Bench support is horizontal, not the case typing angle or desk height'])
    (OUT/'checks.json').write_text(json.dumps(report, indent=2, ensure_ascii=False)+'\n', encoding='utf-8')
    files.update({name: OUT/name for name in ('PRINT-LIST.txt', 'print-list.csv', 'EMAIL-DRAFT.txt',
                 'fixture-plan.svg', 'bench-preview.svg', 'bench-assembly.FCStd', 'checks.json')})
    files['READ-ME-FIRST.txt'] = OUT/'README.md'
    files['holder-preview.svg'] = HOLDER/'preview.svg'
    archive = OUT/'keycult-print-request-PLA.zip'
    # Explicit allowlist excludes upstream/local CAD, personal material and backups.
    with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as zipped:
        for name, path in sorted(files.items()):
            info = zipfile.ZipInfo('keycult-print-request-PLA/'+name, (2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            zipped.writestr(info, path.read_bytes())
    with zipfile.ZipFile(archive) as zipped:
        assert zipped.testzip() is None
        assert set(zipped.namelist()) == {'keycult-print-request-PLA/'+n for n in files}
        for name, path in files.items():
            assert zipped.read('keycult-print-request-PLA/'+name) == path.read_bytes(), name
    print(json.dumps(dict(archive=str(archive), bytes=archive.stat().st_size,
                          stl_files=len(records), printed_pieces=24, fixture_checks=checks), ensure_ascii=False))


if __name__ == '__main__':
    main()
