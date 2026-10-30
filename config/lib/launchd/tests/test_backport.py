from pathlib import Path
import os
import subprocess
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[4]


def home_manager_source():
    expr = f'let f = builtins.getFlake "path:{ROOT}"; in f.inputs.home-manager.outPath + "/modules/launchd/default.nix"'
    return Path(
        subprocess.run(
            ["nix", "eval", "--raw", "--impure", "--expr", expr],
            check=True,
            text=True,
            capture_output=True,
        ).stdout
    )


SOURCE = home_manager_source()


def transformed():
    expr = f"""let f = builtins.getFlake "path:{ROOT}"; in import {ROOT}/config/lib/launchd/backport.nix {{ lib = f.inputs.nixpkgs.lib; source = builtins.readFile {SOURCE}; }}"""
    return subprocess.run(
        ["nix", "eval", "--raw", "--impure", "--expr", expr],
        check=True,
        text=True,
        capture_output=True,
    ).stdout


def body(source):
    begin = source.index("            # Get the OS version")
    end = source.index("          '';", begin)
    return textwrap.dedent(source[begin:end]).replace("''${", "${")


class Fixture:
    def __init__(
        self,
        activation,
        version=26,
        unload=False,
        bootstrap=False,
        dry_run=False,
        loaded=True,
    ):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.new, self.old, self.dst, self.bin = [
            self.root / x
            for x in ("new/LaunchAgents", "old/LaunchAgents", "installed", "bin")
        ]
        for p in (self.new, self.old, self.dst, self.bin):
            p.mkdir(parents=True, exist_ok=True)
        self.log = self.root / "events"
        self.marker = self.root / "failed"
        scripts = {
            "launchctl": 'echo "$*" >> "$FIXTURE_LOG"\n[[ $1 == print && $AGENT_LOADED == 0 ]] && exit 1\n[[ $1 == bootout && $FAIL_UNLOAD == 1 ]] && { echo fatal-unload; exit 1; }\n[[ $1 == bootstrap && $FAIL_BOOTSTRAP == 1 && ! -e $BOOTSTRAP_MARKER ]] && { touch $BOOTSTRAP_MARKER; echo bootstrap-failed; exit 1; }\nexit 0\n',
            "sw_vers": f"echo {version}.0\n",
            "install": 'src=${@: -2:1}; dst=${@: -1}; mkdir -p "$(dirname "$dst")"; cp "$src" "$dst"\n',
            "sleep": 'echo sleep-$* >> "$FIXTURE_LOG"\n',
            "readlink": 'echo "${@: -1}"\n',
        }
        for name, code in scripts.items():
            p = self.bin / name
            p.write_text("#!/usr/bin/env bash\n" + code)
            p.chmod(0o755)
        activation = (
            activation.replace("/bin/launchctl", str(self.bin / "launchctl"))
            .replace("/usr/bin/sw_vers", str(self.bin / "sw_vers"))
            .replace("${lib.escapeShellArg dstDir}", str(self.dst))
        )
        self.script = self.root / "activate"
        self.script.write_text(
            '#!/usr/bin/env bash\nset -e\nrun(){ if [[ -n "${DRY_RUN_CMD:-}" ]]; then return 0; fi; "$@"; }\nverboseEcho(){ :; }\nwarnEcho(){ :; }\nerrorEcho(){ :; }\nVERBOSE_ARG=\'\'\n'
            + f"newGenPath={self.root / 'new'}\noldGenPath={self.root / 'old'}\nPATH={self.bin}:$PATH\n"
            + activation
        )
        self.script.chmod(0o755)
        self.env = {
            "FIXTURE_LOG": str(self.log),
            "FAIL_UNLOAD": str(int(unload)),
            "FAIL_BOOTSTRAP": str(int(bootstrap)),
            "AGENT_LOADED": str(int(loaded)),
            "BOOTSTRAP_MARKER": str(self.marker),
            "DRY_RUN_CMD": "echo" if dry_run else "",
            "PATH": os.environ["PATH"],
        }

    def add(self, where, name, value="old"):
        (getattr(self, where) / f"{name}.plist").write_text(value)

    def run(self):
        return subprocess.run(
            ["bash", str(self.script)], text=True, capture_output=True, env=self.env
        )

    def events(self):
        return self.log.read_text().splitlines() if self.log.exists() else []

    def close(self):
        self.temp.cleanup()


class BackportTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.original, cls.fixed = body(SOURCE.read_text()), body(transformed())

    def fx(self, activation, **kw):
        f = Fixture(activation, **kw)
        self.addCleanup(f.close)
        return f

    def test_fatal_unload_blocks_replacement(self):
        fixed = self.fx(self.fixed, unload=True)
        fixed.add("new", "agent", "new")
        fixed.add("dst", "agent")
        self.assertNotEqual(fixed.run().returncode, 0)
        self.assertEqual((fixed.dst / "agent.plist").read_text(), "old")
        self.assertFalse(any(x.startswith("bootstrap") for x in fixed.events()))

    def test_bootstrap_failure_rolls_back_old_plist_and_reboots(self):
        old = self.fx(self.original, bootstrap=True)
        old.add("new", "agent", "new")
        old.add("dst", "agent")
        self.assertNotEqual(old.run().returncode, 0)
        self.assertEqual((old.dst / "agent.plist").read_text(), "new")
        f = self.fx(self.fixed, bootstrap=True)
        f.add("new", "agent", "new")
        f.add("dst", "agent")
        self.assertNotEqual(f.run().returncode, 0)
        self.assertEqual((f.dst / "agent.plist").read_text(), "old")
        self.assertEqual(sum(x.startswith("bootstrap") for x in f.events()), 2)

    def test_first_bootstrap_failure_removes_new_plist_for_retry(self):
        f = self.fx(self.fixed, bootstrap=True)
        f.add("new", "agent", "new")
        self.assertNotEqual(f.run().returncode, 0)
        self.assertFalse((f.dst / "agent.plist").exists())
        self.assertEqual(f.run().returncode, 0)
        self.assertEqual((f.dst / "agent.plist").read_text(), "new")

    def test_failed_unload_preserves_removed_plist(self):
        f = self.fx(self.fixed, unload=True)
        f.add("old", "gone")
        f.add("dst", "gone")
        self.assertNotEqual(f.run().returncode, 0)
        self.assertTrue((f.dst / "gone.plist").exists())

    def test_successful_removal_and_unchanged_agent(self):
        f = self.fx(self.fixed)
        f.add("old", "gone")
        f.add("dst", "gone")
        f.add("new", "unchanged")
        f.add("dst", "unchanged")
        result = f.run()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((f.dst / "gone.plist").exists())
        self.assertEqual(
            [event for event in f.events() if not event.startswith("print ")],
            [f"bootout --wait gui/{os.getuid()}/gone"],
        )

    def test_domain_migration_failure_restores_installed_plist_in_old_domain(self):
        f = self.fx(self.fixed, bootstrap=True)
        f.add("new", "agent", "new")
        f.add("dst", "agent", "installed")
        f.add("old", "agent", "previous-generation")
        for generation, domain in (("old", "user"), ("new", "gui")):
            domains = f.root / generation / "LaunchAgentDomains"
            domains.mkdir()
            (domains / "agent.domain").write_text(domain + "\n")
        self.assertNotEqual(f.run().returncode, 0)
        self.assertEqual((f.dst / "agent.plist").read_text(), "installed")
        uid = os.getuid()
        self.assertEqual(
            f.events(),
            [
                f"bootout --wait user/{uid}/agent",
                f"bootout --wait gui/{uid}/agent",
                f"bootstrap gui/{uid} {f.dst}/agent.plist",
                f"bootstrap user/{uid} {f.dst}/agent.plist",
            ],
        )

    def test_unchanged_unloaded_agent_is_bootstrapped(self):
        f = self.fx(self.fixed, loaded=False)
        f.add("new", "agent")
        f.add("dst", "agent")
        result = f.run()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(f"bootstrap gui/{os.getuid()} {f.dst}/agent.plist", f.events())

    def test_dry_run_does_not_change_plists_or_leave_backups(self):
        f = self.fx(self.fixed, dry_run=True)
        f.add("new", "changed", "new")
        f.add("dst", "changed")
        f.add("old", "gone")
        f.add("dst", "gone")
        before = {p.name: p.read_bytes() for p in f.dst.iterdir()}
        result = f.run()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(before, {p.name: p.read_bytes() for p in f.dst.iterdir()})
        self.assertEqual(f.events(), [])

    def test_version_gate_and_aggregate_status(self):
        old = self.fx(self.fixed, version=25)
        old.add("new", "a", "new")
        old.add("dst", "a")
        self.assertEqual(old.run().returncode, 0)
        self.assertIn(f"bootout gui/{os.getuid()}/a", old.events())
        self.assertIn("sleep-1", old.events())
        current = self.fx(self.fixed, unload=True)
        current.add("new", "bad", "new")
        current.add("dst", "bad")
        current.add("new", "later", "new")
        self.assertNotEqual(current.run().returncode, 0)
        self.assertIn(f"bootout --wait gui/{os.getuid()}/bad", current.events())
        self.assertTrue(any(x.endswith("later.plist") for x in current.events()))

    def test_source_drift_fails_loudly(self):
        with tempfile.TemporaryDirectory() as d:
            drift = Path(d) / "source"
            drift.write_text(
                SOURCE.read_text().replace("processAgent", "processChangedAgent", 1)
            )
            expr = f"""let f = builtins.getFlake "path:{ROOT}"; in import {ROOT}/config/lib/launchd/backport.nix {{ lib=f.inputs.nixpkgs.lib; source=builtins.readFile {drift}; }}"""
            result = subprocess.run(
                ["nix", "eval", "--raw", "--impure", "--expr", expr],
                text=True,
                capture_output=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("backport anchor did not occur exactly once", result.stderr)
