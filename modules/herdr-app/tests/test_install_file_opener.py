import importlib.util
import os
from pathlib import Path
import plistlib
import subprocess
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("install_file_opener", ROOT / "install-file-opener.py")
installer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(installer)


class FileOpenerInstallation(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.source = self.root / "file-opener.py"
        self.source.write_text("# isolated router fixture\n")
        self.destination = self.root / "Applications/OMP Neovim.app"
        self.backup = self.destination.with_name(self.destination.name + ".home-manager.bak")
        self.commands = []
        self.fail_command = None
        self.real_replace = os.replace
        runner = patch.object(installer.subprocess, "run", side_effect=self.run_command)
        runner.start()
        self.addCleanup(runner.stop)

    def make_bundle(self, path, content, identifier=installer.BUNDLE_ID):
        resources = path / "Contents/Resources"
        resources.mkdir(parents=True)
        (resources / "personal-content").write_text(content)
        with (path / "Contents/Info.plist").open("wb") as file:
            plistlib.dump({"CFBundleIdentifier": identifier}, file)

    def run_command(self, command, *, check):
        self.assertTrue(check)
        self.commands.append(command)
        executable = command[0]
        self.assertIn(executable, ["/usr/bin/osacompile", "/usr/bin/codesign", installer.LSREGISTER])
        if executable == self.fail_command:
            raise subprocess.CalledProcessError(1, command)
        if executable == "/usr/bin/osacompile":
            self.make_bundle(Path(command[2]), "new application", "com.apple.ScriptEditor.id.fixture")
        elif executable == "/usr/bin/codesign":
            (Path(command[-1]) / "Contents/signature").write_text("signed fixture")
        else:
            self.assertTrue((Path(command[-1]) / "Contents/signature").exists())
        return subprocess.CompletedProcess(command, 0)

    def install(self, destination=None):
        installer.install(self.source, "/fixture/python", "/fixture/nvim", destination or self.destination)

    def snapshot(self, path):
        if not path.exists():
            return None
        return {str(item.relative_to(path)): item.read_bytes() if item.is_file() else None
                for item in [path, *sorted(path.rglob("*"))]}

    def assert_no_staging(self, destination=None):
        self.assertEqual(list((destination or self.destination).parent.glob(".omp-file-opener-*")), [])

    def test_first_install_compiles_signs_and_registers_only_custom_scheme(self):
        self.install()
        with (self.destination / "Contents/Info.plist").open("rb") as file:
            info = plistlib.load(file)
        self.assertEqual(info["CFBundleIdentifier"], installer.BUNDLE_ID)
        self.assertEqual(info["CFBundleURLTypes"], [
            {"CFBundleURLName": "OMP file links", "CFBundleURLSchemes": ["omp-nvim"]},
        ])
        self.assertTrue(info["LSUIElement"])
        self.assertFalse(self.backup.exists())
        self.assert_no_staging()

    def test_successful_replacement_keeps_latest_preceding_application_as_backup(self):
        self.make_bundle(self.destination, "current application with personal content")
        self.make_bundle(self.backup, "older application")
        previous = self.snapshot(self.destination)
        self.install()
        self.assertEqual(self.snapshot(self.backup), previous)
        self.assertEqual((self.destination / "Contents/Resources/personal-content").read_text(),
                         "new application")
        self.assert_no_staging()

    def test_unchanged_reinstall_only_registers_and_preserves_existing_backup(self):
        self.install()
        self.make_bundle(self.backup, "older application")
        current, backup = self.snapshot(self.destination), self.snapshot(self.backup)
        modified = (self.destination.stat().st_mtime_ns, self.backup.stat().st_mtime_ns)
        self.commands.clear()
        self.install()
        self.assertEqual(self.snapshot(self.destination), current)
        self.assertEqual(self.snapshot(self.backup), backup)
        self.assertEqual((self.destination.stat().st_mtime_ns, self.backup.stat().st_mtime_ns), modified)
        self.assert_no_staging()

    def test_failed_publication_restores_destination_and_existing_backup(self):
        for existing in [False, True]:
            for backed_up in [False, True]:
                with self.subTest(existing=existing, backed_up=backed_up):
                    destination = self.root / f"publish-{existing}-{backed_up}" / self.destination.name
                    backup = destination.with_name(destination.name + ".home-manager.bak")
                    if existing:
                        self.make_bundle(destination, "previous application")
                    if backed_up:
                        self.make_bundle(backup, "previous backup")
                    before = self.snapshot(destination), self.snapshot(backup)

                    def fail_publish(source, target):
                        if Path(target) == destination and Path(source).name == destination.name:
                            raise OSError("publication denied")
                        return self.real_replace(source, target)

                    with patch.object(installer.os, "replace", side_effect=fail_publish):
                        with self.assertRaisesRegex(OSError, "publication denied"):
                            self.install(destination)
                    self.assertEqual((self.snapshot(destination), self.snapshot(backup)), before)
                    self.assert_no_staging(destination)
        self.assertNotIn(installer.LSREGISTER, [command[0] for command in self.commands])

    def test_failed_registration_restores_destination_and_existing_backup(self):
        self.fail_command = installer.LSREGISTER
        for existing in [False, True]:
            for backed_up in [False, True]:
                with self.subTest(existing=existing, backed_up=backed_up):
                    destination = self.root / f"register-{existing}-{backed_up}" / self.destination.name
                    backup = destination.with_name(destination.name + ".home-manager.bak")
                    if existing:
                        self.make_bundle(destination, "previous application")
                    if backed_up:
                        self.make_bundle(backup, "previous backup")
                    before = self.snapshot(destination), self.snapshot(backup)
                    with self.assertRaises(subprocess.CalledProcessError):
                        self.install(destination)
                    self.assertEqual((self.snapshot(destination), self.snapshot(backup)), before)
                    self.assert_no_staging(destination)

    def test_failed_destination_staging_restores_existing_backup(self):
        self.make_bundle(self.destination, "previous application")
        self.make_bundle(self.backup, "previous backup")
        before = self.snapshot(self.destination), self.snapshot(self.backup)

        def fail_staging(source, target):
            if Path(source) == self.destination:
                raise OSError("destination staging denied")
            return self.real_replace(source, target)

        with patch.object(installer.os, "replace", side_effect=fail_staging):
            with self.assertRaisesRegex(OSError, "destination staging denied"):
                self.install()
        self.assertEqual((self.snapshot(self.destination), self.snapshot(self.backup)), before)
        self.assert_no_staging()

    def test_unrelated_destination_and_backup_are_never_replaced(self):
        for unrelated in [self.destination, self.backup]:
            with self.subTest(unrelated=unrelated.name):
                if not self.destination.exists():
                    self.make_bundle(self.destination, "previous application")
                if unrelated == self.destination:
                    with (unrelated / "Contents/Info.plist").open("wb") as file:
                        plistlib.dump({"CFBundleIdentifier": "unrelated.application"}, file)
                else:
                    with (self.destination / "Contents/Info.plist").open("wb") as file:
                        plistlib.dump({"CFBundleIdentifier": installer.BUNDLE_ID}, file)
                    self.make_bundle(unrelated, "unrelated personal application", "unrelated.application")
                before = self.snapshot(self.destination), self.snapshot(self.backup)
                with self.assertRaisesRegex(ValueError, "Refusing to replace an unrelated"):
                    self.install()
                self.assertEqual((self.snapshot(self.destination), self.snapshot(self.backup)), before)
                self.assert_no_staging()
        self.assertNotIn(installer.LSREGISTER, [command[0] for command in self.commands])

    def test_failed_compile_or_sign_leaves_destination_and_backup_untouched(self):
        self.make_bundle(self.destination, "previous application")
        self.make_bundle(self.backup, "previous backup")
        before = self.snapshot(self.destination), self.snapshot(self.backup)
        for command in ["/usr/bin/osacompile", "/usr/bin/codesign"]:
            with self.subTest(command=command):
                self.fail_command = command
                with self.assertRaises(subprocess.CalledProcessError):
                    self.install()
                self.assertEqual((self.snapshot(self.destination), self.snapshot(self.backup)), before)
                self.assert_no_staging()
        self.assertNotIn(installer.LSREGISTER, [command[0] for command in self.commands])

    def test_failed_rollback_reports_and_retains_recoverable_application(self):
        self.make_bundle(self.destination, "previous application with personal content")
        self.make_bundle(self.backup, "previous backup with personal content")
        current, backup = self.snapshot(self.destination), self.snapshot(self.backup)
        self.fail_command = installer.LSREGISTER

        def fail_restore(source, target):
            if Path(target) == self.destination and Path(source).name == "previous-destination.app":
                raise OSError("restoration denied")
            return self.real_replace(source, target)

        with patch.object(installer.os, "replace", side_effect=fail_restore):
            with self.assertRaisesRegex(RuntimeError, "rollback failed.*restoration denied") as raised:
                self.install()
        recovery = list(self.destination.parent.glob(".omp-file-opener-*"))
        self.assertEqual(len(recovery), 1)
        self.assertIn(str(recovery[0]), str(raised.exception))
        self.assertEqual(self.snapshot(recovery[0] / "previous-destination.app"), current)
        self.assertEqual(self.snapshot(self.backup), backup)
        self.assertFalse(self.destination.exists())


if __name__ == "__main__":
    unittest.main()
