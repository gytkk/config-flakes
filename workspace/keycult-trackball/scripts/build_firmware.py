# /// script
# requires-python = ">=3.11"
# dependencies = ["qmk==1.1.8"]
# ///
"""Check the 62-key electrical map and build its pinned QMK UF2 without flashing."""

import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import tarfile

ROOT = Path(__file__).resolve().parents[1]
QMK_REVISION = "75a037d2f0f9c54bad8e2f68ae82134559c92f22"
QMK = ROOT / ".cache/qmk_firmware"
SOURCE = ROOT / "firmware/keycult62"
OUTPUT = ROOT / ".cache/firmware"


def run(*args, cwd=ROOT, **kwargs):
    return subprocess.run(args, cwd=cwd, check=True, text=True, **kwargs)


def validate_map():
    info = json.loads((SOURCE / "keyboard.json").read_text())
    circuit = json.loads((ROOT / "design/pcb/62/circuit.json").read_text())
    with (ROOT / "design/pcb/62/gpio-map.csv").open() as stream:
        rows = list(csv.DictReader(stream))
    keys = json.loads((ROOT / "design/preliminary/layout.json").read_text())["rear_clearance"]["keys"]
    layout = info["layouts"]["LAYOUT"]["layout"]
    assert len(rows) == len(layout) == len(keys) == circuit["key_count"] == 62
    assert {row["key_id"] for row in rows} == {key["id"] for key in keys}
    assert [entry["matrix"] for entry in layout] == [[int(row["matrix_row"]), int(row["matrix_col"])] for row in rows]
    parts = {part["ref"]: part for part in circuit["parts"]}
    for row in rows:
        r, c = int(row["matrix_row"]), int(row["matrix_col"])
        assert info["matrix_pins"]["rows"][r] == row["row_gpio"] == f"GP{circuit['rows'][r]}"
        assert info["matrix_pins"]["cols"][c] == row["column_gpio"] == f"GP{circuit['columns'][c]}"
        assert row["diode_direction"] == info["diode_direction"] == "COL2ROW"
        assert parts[row["switch"]]["nets"] == {"1": f"COL{c}", "2": f"K_{row['key_id']}"}
        assert parts[row["diode"]]["nets"] == {"1": f"ROW{r}", "2": f"K_{row['key_id']}"}
    assert info["matrix_pins"]["cols"][7] is None and circuit["gpio"]["16"] is None
    assert parts["J2"]["nets"] == {"1": "SENSOR_SCLK_3V3", "2": "SENSOR_CS_3V3", "3": "GND", "4": "+3V3", "5": "GND", "6": "SENSOR_MOSI_3V3", "7": "SENSOR_MISO_3V3", "MP": "GND"}
    config = dict(re.findall(r"^#define (\w+) (\w+)$", (SOURCE / "config.h").read_text(), re.MULTILINE))
    for define, pin, net in (("SPI_SCK_PIN", "GP2", "SENSOR_SCLK_3V3"), ("SPI_MOSI_PIN", "GP3", "SENSOR_MOSI_3V3"), ("SPI_MISO_PIN", "GP0", "SENSOR_MISO_3V3"), ("PMW33XX_CS_PIN", "GP1", "SENSOR_CS_3V3")):
        assert config[define] == pin and circuit["gpio"][pin[2:]] == net
    print("PASS: 62 positions, GPIOs, COL2ROW diode polarity and J2 circuit parity", flush=True)


def validate_uf2(path):
    data = path.read_bytes()
    assert data and len(data) % 512 == 0, "UF2 must contain complete 512-byte blocks"
    blocks = len(data) // 512
    for index in range(blocks):
        block = data[index * 512 : (index + 1) * 512]
        magic0, magic1, flags, address, size, number, total, family = struct.unpack_from("<8I", block)
        assert (magic0, magic1) == (0x0A324655, 0x9E5D5157)
        assert flags & 0x2000 and family == 0xE48BFF56, "Expected RP2040 UF2 family"
        assert number == index and total == blocks and 0 < size <= 476
        assert 0x10000000 <= address < address + size <= 0x10200000
        assert struct.unpack_from("<I", block, 508)[0] == 0x0AB16F30
    return {"bytes": len(data), "blocks": blocks, "sha256": hashlib.sha256(data).hexdigest()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="check source/circuit parity without fetching or building")
    args = parser.parse_args()
    validate_map()
    if args.check:
        return
    for tool in ("git", "make", "arm-none-eabi-gcc"):
        if shutil.which(tool) is None:
            parser.error(f"{tool} is required; see firmware/README.md for the pinned Nix command")
    if not QMK.exists():
        run("git", "clone", "--depth", "1", "--branch", "0.29.0", "https://github.com/qmk/qmk_firmware.git", str(QMK))
    revision = run("git", "rev-parse", "HEAD", cwd=QMK, capture_output=True).stdout.strip()
    if revision != QMK_REVISION:
        parser.error(f"{QMK}: expected {QMK_REVISION}, found {revision}; preserve this checkout and move it before retrying")
    dirty = run("git", "status", "--porcelain", "--untracked-files=no", "--ignore-submodules", cwd=QMK, capture_output=True).stdout
    if dirty:
        parser.error(f"{QMK} has tracked source edits; preserve them before rebuilding")
    run("git", "submodule", "update", "--init", "--depth", "1", "lib/chibios", "lib/chibios-contrib", "lib/pico-sdk", "lib/printf", cwd=QMK)
    submodules = run("git", "submodule", "status", "lib/chibios", "lib/chibios-contrib", "lib/pico-sdk", "lib/printf", cwd=QMK, capture_output=True).stdout
    if any(line[0] != " " for line in submodules.splitlines()):
        parser.error("QMK submodules do not match the pinned gitlinks")
    for name in ("chibios", "chibios-contrib", "pico-sdk", "printf"):
        if run("git", "status", "--porcelain", "--untracked-files=no", cwd=QMK / "lib" / name, capture_output=True).stdout:
            parser.error(f"QMK submodule {name} has source edits; preserve them before rebuilding")
    shutil.copytree(SOURCE, QMK / "keyboards/handwired/keycult62", dirs_exist_ok=True)
    env = os.environ.copy()
    env["SOURCE_DATE_EPOCH"] = run("git", "show", "-s", "--format=%ct", cwd=QMK, capture_output=True).stdout.strip()
    env["QMK_HOME"] = str(QMK)
    run("make", "handwired/keycult62:default", f"PYTHON={sys.executable}", "SKIP_VERSION=yes", "-j2", cwd=QMK, env=env)
    OUTPUT.mkdir(parents=True, exist_ok=True)
    binary = OUTPUT / "keycult62_default.uf2"
    shutil.copy2(QMK / "handwired_keycult62_default.uf2", binary)
    receipt = {"qmk_revision": revision, "qmk_release": "0.29.0", "submodules": submodules.splitlines(), "compiler": run("arm-none-eabi-gcc", "--version", capture_output=True).stdout.splitlines()[0], "source_sha256": {str(path.relative_to(SOURCE)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(SOURCE.rglob("*")) if path.is_file()}, "input_sha256": {path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in ("design/pcb/62/circuit.json", "design/pcb/62/gpio-map.csv", "design/preliminary/layout.json", "scripts/build_firmware.py")}, "uf2": validate_uf2(binary), "hardware_tested": False, "srom_uploaded": False}
    (OUTPUT / "build.json").write_text(json.dumps(receipt, indent=2) + "\n")
    (ROOT / "firmware/build-validation.json").write_text(json.dumps(receipt, indent=2) + "\n")
    shutil.copy2(QMK / "LICENSE", OUTPUT / "QMK-LICENSE.txt")
    archive = OUTPUT / "keycult62-corresponding-source.tar.gz"
    with tarfile.open(archive, "w:gz") as bundle:
        bundle.add(QMK, arcname="qmk_firmware", filter=lambda member: None if {".git", ".build"}.intersection(Path(member.name).parts) or member.name.endswith("handwired_keycult62_default.uf2") else member)
        bundle.add(ROOT / "firmware/README.md", arcname="port/README.md")
        bundle.add(Path(__file__), arcname="port/build_firmware.py")
    print(f"Built and checked {binary}\n{json.dumps(receipt, indent=2)}")
    print(f"Complete matching source and upstream notices: {archive}")


if __name__ == "__main__":
    main()
