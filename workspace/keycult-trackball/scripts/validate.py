# /// script
# requires-python = ">=3.11"
# dependencies = ["shapely==2.1.2", "pymupdf==1.26.7"]
# ///
"""Check independent geometry, artifacts and PDF scale. Not manufacturing DRC."""
from pathlib import Path
from html.parser import HTMLParser
from urllib.parse import unquote, urlsplit
import ast
import itertools
import json
import math
import re
import xml.etree.ElementTree as ET
import pymupdf
from shapely.geometry import box

ROOT = Path(__file__).resolve().parents[1]
DESIGN = ROOT / 'design/preliminary'


def read_json(name):
    return json.loads((DESIGN / name).read_text())


def close(actual, expected, label):
    assert math.isclose(actual, expected, abs_tol=1e-6), (label, actual, expected)


def cap_half(k):
    return (18 + (k['width_u'] - 1) * 19.05) / 2


def check_local_link(path, target):
    parts = urlsplit(target)
    if parts.scheme or parts.netloc or not parts.path or target.startswith('/'):
        return
    linked = (path.parent / unquote(parts.path)).resolve()
    assert linked.is_relative_to(ROOT), (path, target)
    assert linked.exists(), f'Broken local link: {path.relative_to(ROOT)} -> {target}'


class LinkParser(HTMLParser):
    def __init__(self, path):
        super().__init__()
        self.path = path
    def handle_starttag(self, tag, attrs):
        for name, value in attrs:
            if name in ('src', 'href') and value:
                check_local_link(self.path, value)


for path in ROOT.rglob('*'):
    if not path.is_file() or any(p in {'.git', '.venv', '.cache', '__pycache__', 'upstream', 'local'} for p in path.parts):
        continue
    if path.suffix == '.json': json.loads(path.read_text())
    elif path.suffix == '.svg': ET.fromstring(path.read_text())
    elif path.suffix == '.html': LinkParser(path).feed(path.read_text())
    elif path.suffix == '.py': ast.parse(path.read_text())
    elif path.suffix == '.md':
        for link in re.findall(r'\[[^\]]*\]\(([^)]+)\)', path.read_text()):
            check_local_link(path, link)

layout = read_json('layout.json')
report = read_json('validation.json')
plate_width, plate_height, _ = layout['official_plate_envelope_mm']
polygon = box(-plate_width/2, -plate_height/2, plate_width/2, plate_height/2)
assert layout['plate_visualization'].startswith('generic rectangular bounding envelope')
containment = {}
for name, key_count, gap in [('preferred', 65, 3), ('alternative', 64, 2), ('rear_clearance', 62, 3)]:
    data = layout[name]
    keys = data['keys']
    assert len(keys) == key_count == data['key_count']
    assert len({(k['x_mm'], k['y_mm']) for k in keys}) == key_count
    values = report['layouts'][data['name']]
    for i, k in enumerate(keys):
        center_button = gap == 3 and k['id'] in ('r4c6', 'r4c8')
        expected_x = (-14.2875 if k['column'] == 6 else 14.2875) if center_button else (k['column'] - (11 + gap) / 2) * 19.05
        close(k['x_mm'], expected_x, 'x position')
        assert k['width_u'] == (1.5 if center_button else 1)
        assert k['height_u'] == 1
        close(k['y_mm'], (k['row'] - 2) * 19.05, 'y pitch')
    if gap == 3:
        by_id = {k['id']: k for k in keys}
        assert 'r4c7' not in by_id and 'M3' not in {k['label'] for k in keys}
        for key_id, label, x in [('r4c6', 'M1', -14.2875), ('r4c8', 'M2', 14.2875)]:
            key = by_id[key_id]
            assert (key['label'], key['x_mm'], key['y_mm'], key['width_u']) == (label, x, 38.1, 1.5)
            close(2 * cap_half(key), 27.525, '1.5u cap width')
    assert {k['label'] for k in keys} >= set('ABCDEFGHIJKLMNOPQRSTUVWXYZ')
    close(sum(k['x_mm'] for k in keys), 0, 'centering')
    for first, second in itertools.combinations(keys, 2):
        assert abs(first['x_mm'] - second['x_mm']) >= cap_half(first) + cap_half(second) or abs(first['y_mm'] - second['y_mm']) >= 18
    ball_gap = min(math.hypot(max(abs(k['x_mm']) - cap_half(k), 0), max(abs(k['y_mm']) - 9, 0)) - 17 for k in keys)
    keep_gap = min(max(abs(k['x_mm']) - 25 - cap_half(k), abs(k['y_mm']) - 34) for k in keys)
    close(ball_gap, values['ball_to_keycap_min_mm'], 'ball gap')
    close(keep_gap, values['50mm_square_to_keycap_min_signed_mm'], 'keepout gap')
    assert ball_gap > 0
    assert (keep_gap < 0) if name == 'alternative' else (keep_gap > 0)
    containment[name] = {}
    for proxy, half in [('switch_proxy', 7), ('keycap_assumption', 9)]:
        boxes = [(k['id'], box(k['x_mm']-(cap_half(k) if proxy == 'keycap_assumption' else half), k['y_mm']-half, k['x_mm']+(cap_half(k) if proxy == 'keycap_assumption' else half), k['y_mm']+half)) for k in keys]
        outside = [key for key, rectangle in boxes if not polygon.covers(rectangle)]
        assert not outside, (name, proxy, outside)
        containment[name][proxy] = {
            'outside_rectangular_reference': outside,
            'min_clearance_to_rectangular_reference_mm': round(min(rectangle.distance(polygon.boundary) for _, rectangle in boxes), 6),
        }
(DESIGN / 'envelope-containment.json').write_text(json.dumps({
    'scope': 'Generic rectangular bounding envelope ONLY; does not prove official contour or case opening fit.',
    'layouts': containment,
}, indent=2) + '\n')
# Public drawings must not silently reintroduce the copied plate contour.
for name in ('layout.svg', 'alternative.svg', 'print-1to1.svg', 'rear-clearance.svg', 'rear-clearance-print.svg'):
    root = ET.fromstring((DESIGN / name).read_text())
    reference = root.find('.//*[@id="plate-envelope"]')
    assert reference is not None and reference.tag.endswith('rect'), name
    assert float(reference.attrib['width']) == plate_width
    assert float(reference.attrib['height']) == plate_height
    assert all(element.attrib.get('fill', 'none') == 'none'
               for element in root.iter('{http://www.w3.org/2000/svg}path')), name
    if name != 'alternative.svg':
        rectangles = list(root.iter('{http://www.w3.org/2000/svg}rect'))
        assert sum(float(element.attrib['width']) == 27.525 and float(element.attrib['height']) == 18 for element in rectangles) == 2, name
        assert 'M3' not in {element.text for element in root.iter('{http://www.w3.org/2000/svg}text')}, name
assert layout['case_opening_mm'] is None and layout['plate_to_floor_mm'] is None
assert layout['trackball']['holder_verified_envelope_mm'] is None
assert layout['trackball']['ball_center_height_above_plate_mm'] is None

rear = layout['rear_clearance']
original = {k['id']: k for k in layout['preferred']['keys']}
remaining = {k['id']: k for k in rear['keys']}
assert original.keys() - remaining.keys() == {'r0c6', 'r0c7', 'r0c8'}, 'Remove only the three top center keys'
assert all(original[key_id] == key for key_id, key in remaining.items()), 'Other keys must not move or change'
assert not {'Vol-', 'Mute', 'Vol+'} & {k['label'] for k in remaining.values()}
assert layout['trackball']['ball_diameter_mm'] == 34 and layout['trackball']['center_mm'] == [0, 0]
assert layout['trackball']['assembly_keepout_mm'] == [50, 50], 'The measured holder size is still unknown'
assert rear['central_empty_cells_mm'] == {'width': 57.15, 'height': 76.2, 'xmin': -28.575, 'ymin': -47.625, 'xmax': 28.575, 'ymax': 28.575}
empty_grid = box(-28.575, -47.625, 28.575, 28.575)
assert polygon.covers(empty_grid), 'Empty grid exceeds the rectangular reference'
assert all(not empty_grid.intersects(box(k['x_mm']-cap_half(k), k['y_mm']-9, k['x_mm']+cap_half(k), k['y_mm']+9)) for k in remaining.values()), 'A keycap occupies the reserved grid'
for name in ('rear-clearance.svg', 'rear-clearance-print.svg'):
    root = ET.fromstring((DESIGN / name).read_text())
    for element_id, expected in [('rear-clearance-grid', [-28.575, -47.625, 57.15, 76.2]), ('rear-extension', [-28.575, -47.625, 57.15, 19.05])]:
        element = root.find(f'.//*[@id="{element_id}"]')
        assert element is not None, (name, element_id)
        for attr, value in zip(('x', 'y', 'width', 'height'), expected):
            close(float(element.attrib[attr]), value, f'{name} {element_id} {attr}')
    labels = [element.text for element in root.iter('{http://www.w3.org/2000/svg}text')]
    assert not {'Vol-', 'Mute', 'Vol+'} & set(labels), name
    assert any('62 keys' in label for label in labels if label), name

for pdf_name, result_name, key_count in [('print-1to1.pdf', 'print-validation.json', 65), ('rear-clearance-print.pdf', 'rear-clearance-print-validation.json', 62)]:
    with pymupdf.open(DESIGN / pdf_name) as pdf:
        assert len(pdf) == 1, pdf_name
        page = pdf[0]
        assert f'{key_count} keys' in page.get_text(), f'{pdf_name}: wrong layout or stale PDF'
        if key_count == 62:
            assert not any(label in page.get_text() for label in ('Vol-', 'Mute', 'Vol+')), pdf_name
        width, height = page.rect.width * 25.4 / 72, page.rect.height * 25.4 / 72
        assert abs(width - 420) < 1 and abs(height - 297) < 1, (pdf_name, width, height)
        calibration = []
        for drawing in page.get_drawings():
            for item in drawing['items']:
                if item[0] == 'l':
                    start, end = item[1:3]
                    length = abs(end.x - start.x) * 25.4 / 72
                    if abs(end.y - start.y) < .001 and abs(length - 100) < .05:
                        calibration.append(length)
        assert calibration, f'{pdf_name}: no 100mm calibration segment'
        measured = {'page_count': 1, 'page_size_mm': [width, height], 'calibration_horizontal_line_lengths_mm': calibration,
                    'print_driver_scaling': 'user must still print actual size/100%, no fit-to-page'}
        (DESIGN / result_name).write_text(json.dumps(measured, indent=2) + '\n')
        print(f'{pdf_name} calibration: {calibration[0]:.6f} mm.')

print('PASS: Python/JSON/SVG/HTML/link syntax; 65/64/62 keys; 1.5u M1/M2; exact three-key rear removal; unchanged ball; cap/ball/reservation checks; generic rectangular envelope; both A3 PDF scales.')
print('Case opening, mounting stiffness and full 3D assembly remain unverified.')
