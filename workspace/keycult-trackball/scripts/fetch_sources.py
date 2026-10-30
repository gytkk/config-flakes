#!/usr/bin/env python3
"""Fetch only the three examined public source archives; preserve upstream terms."""
from pathlib import Path
import hashlib
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / 'design/preliminary/sources/upstream'
SOURCES = {
    'No160-Plate.zip': (
        'https://resources.keycult.com/plate_files/No160-Plate.zip',
        '3be328eba11d3477b3b38582f3f05cd27e3659f932a92da84854634483111a6d'),
    'No160-Rev1-Plates.zip': (
        'https://resources.keycult.com/plate_files/No160-Rev1-Plates.zip',
        '515c2f6b745b6cd816b4747c9cfb575d02b4db2ce0aa1bff7950f1d48eab2f53'),
    'Trackball_PCB_gerber.zip': (
        'https://raw.githubusercontent.com/Yowkees/keyball/e1d1948e6e60a83f6b3566a209326508200d64ce/keyball46/ball_design_data/Trackball_PCB_gerber.zip',
        '2aa537eb96839a2ab50eac6131db09bdbac6ccb62818b79a50b4a7183c5faf9e'),
}

if __name__ == '__main__':
    DEST.mkdir(parents=True, exist_ok=True)
    for name, (url, expected) in SOURCES.items():
        path = DEST / name
        if path.exists():
            data = path.read_bytes()
        else:
            request = urllib.request.Request(url, headers={'User-Agent': 'keycult-ball-source-verification/1'})
            with urllib.request.urlopen(request, timeout=60) as response:
                data = response.read()
        actual = hashlib.sha256(data).hexdigest()
        if actual != expected:
            raise SystemExit(f'SHA-256 mismatch for {name}: expected {expected}, got {actual}; not written')
        if not path.exists():
            path.write_bytes(data)
        print(f'OK {name}: {actual}')
    print('Local source inspection only; upstream rights unchanged. Archives are git-ignored.')
