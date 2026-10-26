import contextlib
import importlib.util
import io
import json
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock

SPEC = importlib.util.spec_from_file_location(
    "plugin_updates", Path(__file__).parents[1] / "plugin-updates.py"
)
updates = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(updates)


class PluginUpdatesTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.home = Path(temporary.name)
        self.root = self.home / ".claude/plugins"
        self.market = self.root / "marketplaces/example"
        self.source = self.market / "plugins/demo"
        self.source.mkdir(parents=True)
        (self.source / "code.txt").write_text("original")
        self.entry = {"name": "demo", "source": "./plugins/demo", "version": "1.0"}
        self.market_entry = {
            "source": {"source": "directory", "path": str(self.market)},
            "installLocation": str(self.market),
        }
        self.write_catalog()
        self.write(
            self.root / "known_marketplaces.json", {"example": self.market_entry}
        )
        self.cache = self.home / "cache.json"
        self.calls = []
        self.fail = False
        self.output = io.StringIO()
        redirect = contextlib.redirect_stdout(self.output)
        redirect.__enter__()
        self.addCleanup(redirect.__exit__, None, None, None)

    def write(self, path, value):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value))

    def write_catalog(self):
        self.write(
            self.market / ".claude-plugin/marketplace.json", {"plugins": [self.entry]}
        )

    def cli(self, *args):
        self.calls.append(args)
        if self.fail:
            return False
        if args[0] in ("install", "update"):
            destination = self.root / "cache/example/demo/1.0"
            if destination.exists():
                shutil.rmtree(destination)
            shutil.copytree(self.source, destination)
            self.write(
                self.root / "installed_plugins.json",
                {
                    "plugins": {
                        "demo@example": [
                            {
                                "scope": "user",
                                "version": "1.0",
                                "installPath": str(destination),
                            }
                        ]
                    }
                },
            )
        return True

    def updater(self):
        updater = updates.Updater(self.home, self.cache, "/store/claude")
        updater.cli = self.cli
        return updater

    def switch(self):
        self.updater().update(["demo@example"])

    def test_repeat_switch_skips_cli_but_checks_installed_files(self):
        self.switch()
        self.switch()
        self.assertEqual([call[0] for call in self.calls], ["install"])
        installed = self.updater().installed("demo@example")
        (Path(installed["entry"]["installPath"]) / "code.txt").write_text("changed")
        self.switch()
        self.assertEqual(self.calls[-1][0], "update")

    def test_source_content_and_declared_version_changes_invalidate(self):
        self.switch()
        (self.source / "code.txt").write_text("new source")
        self.switch()
        self.entry["version"] = "2.0"
        self.write_catalog()
        self.switch()
        self.assertEqual(len(self.calls), 3)

    def test_deleted_install_is_not_cached(self):
        self.switch()
        installed = self.updater().installed("demo@example")
        shutil.rmtree(installed["entry"]["installPath"])
        self.switch()
        self.assertEqual(self.calls[-1][0], "install")

    def test_executable_mode_change_invalidates_local_source(self):
        self.switch()
        (self.source / "code.txt").chmod(0o755)
        self.switch()
        self.assertEqual(len(self.calls), 2)

    def test_shell_permission_repair_does_not_invalidate_install(self):
        script = self.source / "hook.sh"
        script.write_text("#!/bin/sh\nexit 0\n")
        script.chmod(0o644)
        self.switch()
        installed = self.updater().installed("demo@example")
        (Path(installed["entry"]["installPath"]) / "hook.sh").chmod(0o755)
        self.switch()
        self.assertEqual(len(self.calls), 1)

    def test_versionless_git_plugin_tracks_marketplace_commit(self):
        self.entry.pop("version")
        self.write_catalog()
        self.market_entry["source"] = {"source": "github", "repo": "owner/market"}
        updater = self.updater()
        updater.command = Mock(return_value="a" * 40)
        before = updater.plugin_source("demo@example", self.market_entry)
        updater.command.return_value = "b" * 40
        self.assertNotEqual(
            before, updater.plugin_source("demo@example", self.market_entry)
        )

    def test_project_scope_does_not_satisfy_user_install(self):
        self.switch()
        path = self.root / "installed_plugins.json"
        value = updates.read_json(path)
        value["plugins"]["demo@example"][0]["scope"] = "project"
        self.write(path, value)
        self.switch()
        self.assertEqual(self.calls[-1][0], "install")

    def test_failed_update_invalidates_record_and_retries(self):
        self.switch()
        (self.source / "code.txt").write_text("new source")
        self.fail = True
        self.switch()
        self.assertNotIn("demo@example", updates.read_json(self.cache))
        self.fail = False
        self.switch()
        self.switch()
        self.assertEqual(len(self.calls), 3)

    def test_unknown_source_uses_cli_without_caching(self):
        self.switch()
        self.entry["source"] = {"source": "future-format"}
        self.write_catalog()
        self.switch()
        self.switch()
        self.assertEqual(
            [call[0] for call in self.calls], ["install", "update", "update"]
        )
        self.assertNotIn("demo@example", updates.read_json(self.cache))

    def test_external_source_revision_is_checked_even_when_catalog_unchanged(self):
        self.entry["source"] = {"source": "github", "repo": "owner/plugin"}
        self.write_catalog()
        for revision in ("a" * 40, "a" * 40, "b" * 40):
            updater = self.updater()
            updater.remote = Mock(return_value=revision)
            updater.update(["demo@example"])
            updater.remote.assert_called_once_with(
                "https://github.com/owner/plugin.git", None
            )
        self.assertEqual(len(self.calls), 2)

    def test_pinned_external_source_does_not_check_head(self):
        self.entry["source"] = {
            "source": "git-subdir",
            "url": "https://example.org/plugin.git",
            "path": "plugin",
            "sha": "a" * 40,
        }
        self.write_catalog()
        updater = self.updater()
        updater.remote = Mock(
            side_effect=AssertionError("must not resolve pinned HEAD")
        )
        updater.update(["demo@example"])
        updater.update(["demo@example"])
        self.assertEqual(len(self.calls), 1)

    def test_git_marketplace_only_updates_changed_revision(self):
        self.market_entry["source"] = {"source": "github", "repo": "owner/market"}
        (self.market / ".git").mkdir()
        updater = self.updater()
        updater.remote = Mock(return_value="a" * 40)

        def git(args, timeout=30):
            if "get-url" in args:
                return "git@github.com:owner/market.git"
            if "rev-parse" in args:
                return "a" * 40
            return ""

        updater.command = git
        self.assertTrue(updater.marketplace("example", self.market_entry))
        self.assertEqual(self.calls, [])
        updater.remote.return_value = "b" * 40
        self.assertTrue(updater.marketplace("example", self.market_entry))
        self.assertEqual(self.calls, [("marketplace", "update", "example")])

    def test_marketplace_failure_prevents_plugin_cache_hit(self):
        self.switch()
        updater = self.updater()
        updater.marketplace = Mock(return_value=False)
        updater.update(["demo@example"])
        self.assertEqual(len(self.calls), 2)
        self.assertNotIn("demo@example", updates.read_json(self.cache))

    def test_remote_failure_falls_back_to_marketplace_update(self):
        self.market_entry["source"] = {"source": "github", "repo": "owner/market"}
        (self.market / ".git").mkdir()
        updater = self.updater()
        updater.command = Mock(side_effect=subprocess.TimeoutExpired("git", 30))
        updater.marketplace("example", self.market_entry)
        self.assertEqual(self.calls, [("marketplace", "update", "example")])

    def test_archive_marketplace_caches_revision_and_local_contents(self):
        self.market_entry["source"] = {"source": "github", "repo": "owner/market"}
        for revision in ("a" * 40, "a" * 40, "b" * 40):
            updater = self.updater()
            updater.remote = Mock(return_value=revision)
            self.assertTrue(updater.marketplace("example", self.market_entry))
        self.assertEqual(len(self.calls), 2)
        (self.source / "code.txt").write_text("modified")
        updater = self.updater()
        updater.remote = Mock(return_value="b" * 40)
        updater.marketplace("example", self.market_entry)
        self.assertEqual(len(self.calls), 3)

    def test_failed_archive_update_is_retried(self):
        self.market_entry["source"] = {"source": "github", "repo": "owner/market"}
        self.fail = True
        for _ in range(2):
            updater = self.updater()
            updater.remote = Mock(return_value="a" * 40)
            self.assertFalse(updater.marketplace("example", self.market_entry))
        self.assertEqual(len(self.calls), 2)
        self.assertNotIn("marketplace:example", updates.read_json(self.cache))

    def test_corrupt_cache_and_changed_claude_binary_recheck_plugins(self):
        self.switch()
        self.cache.write_text("not JSON")
        self.switch()
        updater = self.updater()
        updater.claude = "/new-store/claude"
        updater.update(["demo@example"])
        self.assertEqual(len(self.calls), 3)

    def test_remote_revision_is_memoized_within_invocation(self):
        updater = self.updater()
        updater.command = Mock(return_value="a" * 40 + "\tHEAD")
        self.assertEqual(updater.remote("url"), "a" * 40)
        self.assertEqual(updater.remote("url"), "a" * 40)
        self.assertEqual(updater.command.call_count, 1)


if __name__ == "__main__":
    unittest.main()
