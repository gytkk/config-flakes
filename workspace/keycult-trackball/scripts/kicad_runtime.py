"""Locate installed KiCad libraries/API, including the existing Nix wrapper.

No downloads or global configuration changes. Standard installations use their
normal Python module and KICAD10_* environment variables.
"""
import os
import re
import shutil
import sys
from pathlib import Path


def configure():
    cli = shutil.which('kicad-cli')
    if not cli:
        raise RuntimeError('KiCad 10 kicad-cli is required')
    wrapper = Path(cli).resolve()
    try:
        text = wrapper.read_text()
    except UnicodeDecodeError:
        text = ''
    for key in ('KICAD10_SYMBOL_DIR', 'KICAD10_FOOTPRINT_DIR', 'KICAD10_3DMODEL_DIR'):
        match = re.search(r'export ' + key + r"=\$\{" + key + r"-'([^']+)'\}", text)
        if match:
            os.environ.setdefault(key, match[1])
    match = re.search(r"export PYTHONPATH='([^']+)'", text)
    if match:
        sys.path.extend(match[1].split(':'))
    for key, leaf in [('KICAD10_SYMBOL_DIR', 'symbols'), ('KICAD10_FOOTPRINT_DIR', 'footprints')]:
        if key not in os.environ:
            for base in ('/usr/share/kicad', '/usr/local/share/kicad'):
                if Path(base, leaf).exists():
                    os.environ[key] = str(Path(base, leaf))
                    break
    return cli
