# /// script
# requires-python = ">=3.11"
# dependencies = ["ezdxf==1.4.3"]
# ///
"""Add an explicitly assumed case to copies of the local holder studies."""
import json
import math

from build_plate import App, Part, ROOT, V, feature, sha

LOCAL = ROOT / "design/preliminary/sources/local"
OUT = LOCAL / "case-study"
SOURCE = "https://resources.keycult.com/specs_no_1_60_65.html"
EXTERIOR = dict(width=294.3, depth=111., front_height=20.35, rear_height=35.8)
# shortcut: rectangular cavity and bezel; replace with registered case CAD or measurements before fit approval.
ASSUMED = dict(wall=3., floor=3., rim_above_plate=5., bezel_thickness=4.,
               opening_width=286.5, opening_depth=96.25)
PARTS = ("Plate", "Basket", "Ball", "ConnectorBoardProxy", "SensorBoardProxy",
         "HolderScrewHeadsProxy", "ConnectorScrewHeadsProxy", "FlangeFastenersProxy",
         "SwitchBodyProxies", "AllKeycapProxies", "CurrentPCB", "ProposedOpenPCB",
         "HolderEnvelopeAssumption")


def box(x, y, z, w, d, h):
    return Part.makeBox(w, d, h, V(x, y, z))


def valid(shape, name):
    if not shape.isValid() or len(shape.Solids) != 1 or shape.Volume <= 0:
        raise ValueError("Expected one valid solid: " + name)


def case_geometry(settings):
    e, s = EXTERIOR, settings
    w, d = e['width'], e['depth']
    if not (0 < s['wall'] < min(w, d)/2 and 0 < s['floor'] < e['front_height']
            and 0 < s['bezel_thickness'] < e['front_height']
            and 0 < s['opening_width'] < w-2*s['wall']
            and 0 < s['opening_depth'] < d-2*s['wall']):
        raise ValueError("Invalid assumed case dimensions")
    mid = (e['front_height']+e['rear_height'])/2
    angle = math.atan2(e['rear_height']-e['front_height'], d)
    rotation = App.Rotation(V(1, 0, 0), math.degrees(angle))
    placement = App.Placement(V(0, 0, s['rim_above_plate'])-rotation.multVec(V(0, 0, mid)), rotation)
    points = [V(-w/2, -d/2, 0), V(-w/2, d/2, 0),
              V(-w/2, d/2, e['front_height']), V(-w/2, -d/2, e['rear_height'])]
    outer = Part.Face(Part.makePolygon(points+[points[0]])).extrude(V(w, 0, 0))
    valid(outer, 'desk-frame exterior')
    assert abs(outer.BoundBox.XLength-w) < 1e-7 and abs(outer.BoundBox.YLength-d) < 1e-7
    for y, height in ((-d/2, e['rear_height']), (d/2, e['front_height'])):
        assert abs(placement.multVec(V(0, y, height)).z-s['rim_above_plate']) < 1e-7
    assert placement.multVec(V(0, d/2, 0)).z > placement.multVec(V(0, -d/2, 0)).z
    outer.Placement = placement
    cavity = box(-w/2+s['wall'], -d/2+s['wall'], s['floor'],
                 w-2*s['wall'], d-2*s['wall'], 100)
    cavity.Placement = placement
    ceiling = s['rim_above_plate']-s['bezel_thickness']
    cavity = cavity.common(box(-200, -100, -100, 400, 200, 100+ceiling))
    valid(cavity, 'assumed cavity')
    aperture = box(-s['opening_width']/2, -s['opening_depth']/2, ceiling,
                   s['opening_width'], s['opening_depth'], 100)
    shell = outer.cut(cavity.fuse(aperture)).removeSplitter()
    valid(shell, 'assumed case shell')
    floor = placement.multVec(V(0, 0, s['floor']))
    floor_z_at_center = floor.z-math.tan(angle)*floor.y
    return shell, dict(derived_slope_degrees=math.degrees(angle),
                       flattened_rim_depth_mm=d/math.cos(angle),
                       assumed_inner_floor_z_at_layout_center_mm=floor_z_at_center)


def overlap_record(shape, shell):
    intersection = shape.common(shell)
    volume = intersection.Volume
    distance = shape.distToShape(shell)[0]
    if not intersection.isNull() and not intersection.isValid():
        raise ValueError("Invalid case intersection")
    status = 'overlap' if volume > 1e-6 else ('touching' if distance < 1e-6 else 'clear')
    return dict(status=status, overlap_mm3=volume, minimum_distance_mm=distance), intersection


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    layout_path = ROOT / 'design/preliminary/layout.json'
    layout_digest = sha(layout_path)
    layout = json.loads(layout_path.read_text())
    holder_report = json.loads((LOCAL / 'holder-study/review.json').read_text())
    if holder_report['layout_sha256'] != layout_digest:
        raise ValueError('Holder study layout is stale; rebuild the holder study first')
    shell, calculated = case_geometry(ASSUMED)
    # Independent inside/outside probes catch overlap and distance classification regressions.
    for probe, expected in ((box(-1, -1, -10, 2, 2, 2), 'clear'),
                            (box(145, -1, -5, 1, 1, 1), 'overlap')):
        assert overlap_record(probe, shell)[0]['status'] == expected
    reports = []
    for selector, key in ((65, 'preferred'), (62, 'rear_clearance')):
        pcb = ROOT / 'design/pcb' / ('62' if selector == 62 else '') / f'keycult{selector}.kicad_pcb'
        holder_variant = next(v for v in holder_report['variants'] if v['selector'] == selector)
        expected = holder_variant['pcb_sha256']
        if sha(pcb) != expected:
            raise ValueError(f'Holder study PCB is stale: {pcb}')
        opening = layout[key].get('pcb_opening')
        if holder_variant.get('pcb_opening') != opening:
            raise ValueError('Holder study native PCB opening is stale; rebuild the holder study first')
        source = LOCAL / f'holder-study/{selector}/holder-study.FCStd'
        source_hash = sha(source)
        folder = OUT / str(selector)
        folder.mkdir(parents=True, exist_ok=True)
        doc = App.openDocument(str(source))
        original_volumes = {o.Name: o.Shape.Volume for o in doc.Objects if hasattr(o, 'Shape')}
        doc.Label = f'No.1-60 {selector} keys - ASSUMED case'
        metadata = doc.addObject('App::FeaturePython', 'CaseReferenceParameters')
        metadata.addProperty('App::PropertyString', 'Source').Source = SOURCE
        metadata.addProperty('App::PropertyString', 'ExteriorMM').ExteriorMM = json.dumps(EXTERIOR)
        metadata.addProperty('App::PropertyString', 'AssumptionsMM').AssumptionsMM = json.dumps(ASSUMED)
        metadata.addProperty('App::PropertyString', 'NativePCBOpening').NativePCBOpening = json.dumps(opening)
        metadata.addProperty('App::PropertyString', 'Registration').Registration = (
            'Plate-local X right, Y front, Z up. Case top midpoint aligned to XY=0, rim Z=5 assumed. '
            'Exterior heights define an illustrative wedge, not a verified typing angle or plate datum.')
        metadata.addProperty('App::PropertyString', 'Warning').Warning = (
            'ASSUMED INTERIOR, NOT A FIT APPROVAL. Historical exterior reference only; owned revision unverified. '
            'No raised mid-piece floor, ribs, bosses, gasket pockets, weight, daughterboard, cable or assembly-path model. '
            'The simplified cavity may overestimate usable depth. '
            'Properties record inputs; edit build_case_study.py and regenerate to change geometry.')
        feature(doc, 'CaseShellAssumption', shell, metadata.Warning)
        section = shell.common(box(-200, -100, -100, 200, 200, 200))
        valid(section, 'half case')
        feature(doc, 'CaseCutawayAssumption', section, 'Left half of assumed shell; visualization only')
        caps = []
        for k in layout[key]['keys']:
            width = 18+(k.get('width_u', 1)-1)*19.05
            caps.append(box(k['x_mm']-width/2, k['y_mm']-9, 5, width, 18, 10))
        feature(doc, 'AllKeycapProxies', Part.makeCompound(caps),
                'All current key positions; rectangular keycaps at Z=5..15 assumed; travel not modeled')
        results, intersections = {}, []
        for name in PARTS:
            obj = doc.getObject(name)
            if obj is None:
                raise ValueError('Missing source object: '+name)
            results[name], common = overlap_record(obj.Shape, shell)
            if results[name]['status'] == 'overlap':
                intersections.append(common)
        if intersections:
            feature(doc, 'CaseOverlapHighlights', Part.makeCompound(intersections),
                    'Red intersections against ASSUMED shell; PCB alternatives and holder envelope checked separately in report')
        report = dict(selector=selector, status='Conditional geometry only; actual case fit unknown',
                      source_fcstd_sha256=source_hash, layout_sha256=layout_digest, pcb_sha256=expected,
                      source=SOURCE, checked='2026-10-30', exterior_mm=EXTERIOR,
                      assumptions_mm=ASSUMED, calculated=calculated, parts=results,
                      pcb_opening=opening,
                      pcb_note=('CurrentPCB has the validated native rounded opening; ProposedOpenPCB is a sharp-corner comparison'
                                if opening else 'CurrentPCB is uncut; ProposedOpenPCB is a sharp-corner comparison only'),
                      omitted=metadata.Warning)
        metadata.addProperty('App::PropertyString', 'ResultsJSON').ResultsJSON = json.dumps(results)
        doc.recompute()
        target = folder / 'case-study.FCStd'
        doc.saveAs(str(target))
        App.closeDocument(doc.Name)
        reopened = App.openDocument(str(target))
        valid(reopened.getObject('CaseShellAssumption').Shape, 'saved case')
        for name, volume in original_volumes.items():
            assert abs(reopened.getObject(name).Shape.Volume-volume) < 1e-6, name
        App.closeDocument(reopened.Name)
        if sha(source) != source_hash or sha(pcb) != expected or sha(layout_path) != layout_digest:
            raise ValueError('Inputs changed during case generation')
        (folder / 'checks.json').write_text(json.dumps(report, indent=2)+'\n')
        reports.append(report)
        print(f'Saved {target}; overlaps: '+', '.join(n for n, r in results.items() if r['status'] == 'overlap'), flush=True)
    (OUT / 'review.json').write_text(json.dumps(reports, indent=2)+'\n')


if __name__ == '__main__':
    main()
