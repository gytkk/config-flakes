# /// script
# requires-python = ">=3.11"
# dependencies = ["ezdxf==1.4.3", "shapely==2.1.2"]
# ///
"""Reproduce historical perimeter checks locally; never publish upstream contours.

Read upstream terms and run fetch_sources.py first. No network or archive
extraction is performed here. Coordinates/overlay output stays git-ignored.
"""
from pathlib import Path
import hashlib
import io
import json
import math
import re
import zipfile
import ezdxf
from ezdxf.path import make_path
from shapely.geometry import Polygon, box
from fetch_sources import SOURCES

ROOT = Path(__file__).resolve().parents[1]
DESIGN = ROOT / 'design/preliminary'
LOCAL = DESIGN / 'sources/local'
archive = DESIGN / 'sources/upstream/No160-Plate.zip'
if not archive.exists():
    raise SystemExit('Read upstream terms, then run python3 scripts/fetch_sources.py first.')
blob = archive.read_bytes()
assert hashlib.sha256(blob).hexdigest() == SOURCES[archive.name][1], 'Archive SHA-256 mismatch'
with zipfile.ZipFile(io.BytesIO(blob)) as z:
    raw_dxf = z.read('No160-Plate.dxf')
assert hashlib.sha256(raw_dxf).hexdigest() == 'cf77d96bb3a390868e65b2a3c16c4968322974e50347dd253ace4be5debb0031', 'DXF SHA-256 mismatch'
doc = ezdxf.read(io.StringIO(raw_dxf.decode('utf-8'), newline=None))
assert doc.header['$INSUNITS'] == 4, 'Expected millimetres'
candidates = []
for entity in doc.modelspace().query('LWPOLYLINE'):
    if not entity.closed:
        continue
    native = list(make_path(entity).flattening(0.01))
    polygon = Polygon([(point.x, point.y) for point in native])
    if polygon.is_valid:
        candidates.append((polygon.area, entity, native))
assert candidates, 'No valid closed DXF polyline'
_, outline_entity, native = max(candidates, key=lambda item: item[0])
outline = [[-point.x - 142.875, point.y - 47.625] for point in native]
polygon = Polygon(outline)
assert polygon.is_valid
for value, expected in zip(polygon.bounds, [-142.875, -54.125, 142.875, 54.125]):
    assert math.isclose(value, expected, abs_tol=1e-6), polygon.bounds
layout = json.loads((DESIGN / 'layout.json').read_text())
containment = {}
for name in ('preferred', 'alternative', 'rear_clearance'):
    containment[name] = {}
    for proxy, half in [('switch_proxy', 7), ('keycap_assumption', 9)]:
        rectangles = []
        for k in layout[name]['keys']:
            half_width = half + (k['width_u'] - 1) * layout['pitch_mm'] / 2 if proxy == 'keycap_assumption' else half
            rectangles.append((k['id'], box(k['x_mm']-half_width, k['y_mm']-half, k['x_mm']+half_width, k['y_mm']+half)))
        outside = [key for key, rectangle in rectangles if not polygon.covers(rectangle)]
        assert not outside, (name, proxy, outside)
        containment[name][proxy] = {
            'outside_original_plate_contour': outside,
            'min_clearance_to_original_plate_outer_edge_mm': round(min(rectangle.distance(polygon.boundary) for _, rectangle in rectangles), 6),
        }
LOCAL.mkdir(parents=True, exist_ok=True)
measurement = {
    'source_file': 'No160-Plate.dxf',
    'source_archive_sha256': hashlib.sha256(blob).hexdigest(),
    'dxf_units_code': 4, 'unit': 'mm',
    'plot_transform': 'x=-x_native-142.875; y=y_native-47.625; physical datum unverified',
    'outline_flatten_tolerance_mm': 0.01,
    'outline_mm': outline,
    'native_vertices_xy_bulge': [list(point) for point in outline_entity.get_points('xyb')],
}
(LOCAL / 'measured-official-dxf.json').write_text(json.dumps(measurement, indent=2) + '\n')
(LOCAL / 'plate-containment.json').write_text(json.dumps(containment, indent=2) + '\n')
svg = (DESIGN / 'layout.svg').read_text()
contour = '<path d="M' + ' L'.join(f'{x:.4f},{y:.4f}' for x, y in outline) + ' Z" fill="#ffffff" stroke="#0f766e" stroke-width=".38"/>'
svg, replacements = re.subn(r'<rect id="plate-envelope" [^>]*/>', contour, svg)
assert replacements == 1, 'Expected one generic plate reference to replace'
svg = svg.replace('Green: 285.75 × 108.25 bounding reference ONLY — not the official contour.', 'Green: original DXF perimeter — LOCAL inspection only; opening unverified.')
(LOCAL / 'layout-overlay.svg').write_text(svg)
historical = json.loads((DESIGN / 'plate-containment.json').read_text())
assert {name: containment[name] for name in historical} == historical, 'Current local result differs from historical summary; inspect sources/local/plate-containment.json before updating the historical report.'
print('PASS: pinned official DXF checksum, units, outline envelope, all proxies inside perimeter, historical scalar results reproduced.')
print('Detailed contour and overlay written under ignored design/preliminary/sources/local/.')
print('This verifies only the original plate perimeter, NOT the case opening, revision registration or 3D assembly.')
