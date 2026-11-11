"""Install the OMP-only macOS URL handler, without changing file associations."""

import argparse
import hashlib
import os
from pathlib import Path
import plistlib
import shlex
import shutil
import subprocess
import tempfile

BUNDLE_ID = "dev.gytkk.omp-nvim"
LSREGISTER = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"


def install(source: Path, python: str, nvim: str, destination: Path):
    command = shlex.join([python, str(source), "open", "--nvim", nvim])
    literal = '"' + command.replace("\\", "\\\\").replace('"', '\\"') + '"'
    script = f'''on open location fileLink
    try
        do shell script {literal} & " " & quoted form of fileLink
    on error errorMessage
        display alert "OMP file could not be opened" message errorMessage
    end try
end open location
'''
    stamp = hashlib.sha256(script.encode() + source.read_bytes() + Path(__file__).read_bytes()).hexdigest()
    marker = destination / "Contents/Resources/managed-version"
    if destination.exists():
        with (destination / "Contents/Info.plist").open("rb") as file:
            info = plistlib.load(file)
        if info.get("CFBundleIdentifier") != BUNDLE_ID:
            raise ValueError(f"Refusing to replace an unrelated application: {destination}")
    if not marker.exists() or marker.read_text() != stamp:
        destination.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix=".omp-file-opener-", dir=destination.parent) as directory:
            directory = Path(directory)
            source_script = directory / "handler.applescript"
            source_script.write_text(script)
            bundle = directory / destination.name
            subprocess.run(["/usr/bin/osacompile", "-o", str(bundle), str(source_script)], check=True)
            plist = bundle / "Contents/Info.plist"
            with plist.open("rb") as file:
                info = plistlib.load(file)
            info.update({
                "CFBundleIdentifier": BUNDLE_ID,
                "CFBundleName": "OMP Neovim",
                "CFBundleURLTypes": [{"CFBundleURLName": "OMP file links", "CFBundleURLSchemes": ["omp-nvim"]}],
                "LSUIElement": True,
            })
            with plist.open("wb") as file:
                plistlib.dump(info, file)
            (bundle / "Contents/Resources/managed-version").write_text(stamp)
            subprocess.run(["/usr/bin/codesign", "--force", "--sign", "-", str(bundle)], check=True)
            if destination.exists():
                backup = destination.with_name(destination.name + ".home-manager.bak")
                if backup.exists():
                    with (backup / "Contents/Info.plist").open("rb") as file:
                        if plistlib.load(file).get("CFBundleIdentifier") != BUNDLE_ID:
                            raise ValueError(f"Refusing to replace an unrelated backup: {backup}")
                    shutil.rmtree(backup)
                os.replace(destination, backup)
            os.replace(bundle, destination)
    subprocess.run([LSREGISTER, "-f", str(destination)], check=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("python")
    parser.add_argument("nvim")
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    install(args.source.resolve(), args.python, args.nvim, args.destination)
