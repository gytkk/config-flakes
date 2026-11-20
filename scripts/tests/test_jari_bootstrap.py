"""Exercise bootstrap's preflight without touching a provisioned worker."""
import os
from pathlib import Path
import shlex
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


class BootstrapTest(unittest.TestCase):
    @unittest.skipIf(os.geteuid() == 0, "requires an unprivileged caller")
    def test_private_worker_directory(self):
        with tempfile.TemporaryDirectory() as tmp:
            etc = Path(tmp) / "etc"
            worker = etc / "nixos/jari"
            worker.mkdir(parents=True)
            (etc / "NIXOS").touch()
            module = worker / "default.nix"
            module.touch()
            script = (ROOT / "systems/base/jari/bootstrap.sh").read_text().replace("/etc/", f"{etc}/")
            # Simulate the privilege boundary for the read-only test. Stop at the checkout check.
            setup = f'''
                id() {{ echo jari; }}
                sudo() {{
                    [ "$1" = test ] || return 1
                    chmod 700 {shlex.quote(str(worker))}
                    "$@"
                }}
                git() {{ echo dirty; }}
            '''
            for installed in (True, False):
                with self.subTest(installed=installed):
                    if not installed:
                        module.unlink()
                    worker.chmod(0)
                    try:
                        result = subprocess.run(["bash", "-c", setup + script], capture_output=True, text=True)
                    finally:
                        worker.chmod(0o700)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("Review and commit" if installed else str(module), result.stderr)
