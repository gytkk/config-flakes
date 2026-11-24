"""Opt-in CLI contract checks: AGENT_SKILLS_NETWORK=1 uv run --project agent-core -m pytest scripts/tests/test_skills_cli_network.py -v.

Requires Node, npm, Git and network access. All commands use temporary homes and
never execute skill helpers. Command output and resolved revisions remain under
pytest's temporary directory for diagnosis.
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import re
import shutil
import subprocess
from pathlib import Path

import pytest
from agent_core.skill_sources import installation_plan

pytestmark = pytest.mark.skipif(
    os.environ.get("AGENT_SKILLS_NETWORK") != "1",
    reason="set AGENT_SKILLS_NETWORK=1 to acquire and run skills@1.7.2",
)


class Probe:
    def __init__(self, root: Path, cli: Path | None = None):
        self.root = root.resolve()
        self.root.mkdir(parents=True, exist_ok=True)
        self.env = {"PATH": os.environ["PATH"]}
        for variable, relative in {
            "HOME": "home",
            "XDG_CONFIG_HOME": "xdg/config",
            "XDG_DATA_HOME": "xdg/data",
            "XDG_CACHE_HOME": "xdg/cache",
            "XDG_STATE_HOME": "xdg/state",
            "npm_config_cache": "npm-cache",
            "TMPDIR": "tmp",
        }.items():
            directory = self.root / relative
            directory.mkdir(parents=True, exist_ok=True)
            self.env[variable] = str(directory)
        self.env.update(
            npm_config_userconfig=os.devnull,
            GIT_CONFIG_GLOBAL=os.devnull,
            GIT_CONFIG_NOSYSTEM="1",
            GIT_TERMINAL_PROMPT="0",
            DISABLE_TELEMETRY="1",
            DO_NOT_TRACK="1",
            LC_ALL="C",
            LANG="C",
        )
        self.home = Path(self.env["HOME"])
        self.workspace = self.root / "workspace"
        self.workspace.mkdir()
        self.logs = self.root / "logs"
        self.logs.mkdir()
        self.cli = cli or self.root / "tool/node_modules/skills/bin/cli.mjs"

    def run(self, label, command, cwd=None, env=None):
        result = subprocess.run(
            command,
            check=False,
            cwd=cwd or self.workspace,
            env=env or self.env,
            text=True,
            capture_output=True,
            timeout=300,
        )
        (self.logs / f"{label}.json").write_text(
            json.dumps(
                {
                    "command": [str(part) for part in command],
                    "cwd": str(cwd or self.workspace),
                    "exit": result.returncode,
                    "stdout": result.stdout,
                    "stderr": result.stderr,
                },
                indent=2,
            )
            + "\n"
        )
        return result

    def checked(self, label, command, cwd=None):
        result = self.run(label, command, cwd)
        assert result.returncode == 0, (
            f"{label} exited {result.returncode}; evidence: {self.logs}\n"
            f"{result.stdout}\n{result.stderr}"
        )
        return result

    def add(self, label, source, names, target="shared", json_mode=True):
        cwd = self.workspace
        agents = {
            "shared": ["codex", "pi"],
            "claude": ["claude-code"],
        }[target]
        flags = [] if target == "shared" else ["--copy"]
        flags.append("--global")
        command = [
            "node",
            str(self.cli),
            "add",
            str(source),
            "--skill",
            *names,
            "--agent",
            *agents,
            *flags,
            "--yes",
        ]
        if json_mode:
            command.append("--json")
        return self.run(label, command, cwd)

    def destination(self, target):
        return (
            self.home
            / {
                "shared": ".agents/skills",
                "claude": ".claude/skills",
            }[target]
        )

    @property
    def lock(self):
        return Path(self.env["XDG_STATE_HOME"]) / "skills/.skill-lock.json"


def files(directory):
    """Compare complete ordinary-file trees, including helper and metadata bytes."""
    assert directory.is_dir() and not directory.is_symlink(), directory
    result = {}
    for path in sorted(directory.rglob("*")):
        assert not path.is_symlink(), path
        if path.is_dir():
            continue
        assert path.is_file() and path.stat().st_nlink == 1, path
        result[path.relative_to(directory).as_posix()] = path.read_bytes()
    return result


def rows(result, names, target, destination, revision):
    assert result.returncode == 0, f"{result.stdout}\n{result.stderr}"
    value = json.loads(result.stdout)
    assert isinstance(value, list) and len(value) == len(names), value
    assert {row.get("name") for row in value} == set(names), value
    expected_agents = {
        "shared": ["Codex", "Pi"],
        "claude": ["Claude Code"],
    }[target]
    for row in value:
        assert row["status"] == "installed", row
        assert row["agents"] == expected_agents, row
        assert row["mode"] == "copy", row
        assert row["scope"] == "global", row
        assert Path(row["path"]).resolve() == (destination / row["name"]).resolve(), row
        assert row["ref"] == revision, row
        assert re.fullmatch(r"[0-9a-f]{64}", row["hash"]), row
    return value


@pytest.fixture(scope="module")
def cli_probe(tmp_path_factory):
    for command in ["node", "npm", "git"]:
        assert shutil.which(command), f"network integration requires {command}"
    probe = Probe(tmp_path_factory.mktemp("skills-cli-network"))
    probe.checked(
        "npm-install",
        [
            "npm",
            "install",
            "--prefix",
            str(probe.root / "tool"),
            "--ignore-scripts",
            "--no-audit",
            "--no-fund",
            "skills@1.7.2",
        ],
    )
    result = probe.checked("version", ["node", str(probe.cli), "--version"])
    assert result.stdout.strip() == "1.7.2"
    return probe


@pytest.fixture(scope="module")
def source_snapshots(cli_probe):
    probe = cli_probe
    plans = {
        target: installation_plan(target) for target in ["shared", "claude"]
    }
    assert all(plan["cli_version"] == "1.7.2" for plan in plans.values())
    sources = {
        source["id"]: source for plan in plans.values() for source in plan["sources"]
    }
    revisions = {}
    roots = {}
    inventory = {}
    missing = []
    for source_id, source in sources.items():
        if source["kind"] == "local":
            roots[source_id] = Path(source["path"])
            continue
        result = probe.checked(
            source_id + "-head", ["git", "ls-remote", "--symref", source["url"], "HEAD"]
        )
        heads = [
            line.split()[0]
            for line in result.stdout.splitlines()
            if line.endswith("\tHEAD") and not line.startswith("ref:")
        ]
        assert len(heads) == 1 and re.fullmatch(r"[0-9a-f]{40}", heads[0]), (
            result.stdout
        )
        revision = revisions[source_id] = heads[0]
        checkout = roots[source_id] = probe.root / "sources" / source_id
        probe.checked(source_id + "-init", ["git", "init", str(checkout)])
        probe.checked(
            source_id + "-remote",
            ["git", "remote", "add", "origin", source["url"]],
            checkout,
        )
        probe.checked(
            source_id + "-fetch",
            ["git", "fetch", "--depth", "1", "origin", revision],
            checkout,
        )
        probe.checked(
            source_id + "-checkout",
            ["git", "checkout", "--detach", "FETCH_HEAD"],
            checkout,
        )
        actual = probe.checked(
            source_id + "-rev", ["git", "rev-parse", "HEAD"], checkout
        )
        assert actual.stdout.strip() == revision
        license_file = checkout / source["license"]
        assert license_file.is_file() and license_file.read_bytes().strip(), (
            license_file
        )
        inventory[source_id] = {
            "revision": revision,
            "license": source["license"],
            "license_sha256": hashlib.sha256(license_file.read_bytes()).hexdigest(),
            "skills": {},
        }
    # Union per-source selections across plans; no missing upstream skill is omitted.
    for plan in plans.values():
        for source in plan["sources"]:
            for name, relative in source["skills"].items():
                skill = roots[source["id"]] / relative
                if not (skill / "SKILL.md").is_file():
                    missing.append(f"{source['id']}:{name}:{relative}")
                    continue
                tree = files(skill)
                assert re.search(
                    rf"(?m)^name:\s*{re.escape(name)}\s*$", tree["SKILL.md"].decode()
                ), skill
                if source["kind"] == "git":
                    inventory[source["id"]]["skills"][name] = {
                        "path": relative,
                        "files": {
                            path: hashlib.sha256(data).hexdigest()
                            for path, data in tree.items()
                        },
                        "ui_metadata": tree.get("agents/openai.yaml", b"").decode(),
                    }
    (probe.root / "source-inventory.json").write_text(
        json.dumps(inventory, indent=2) + "\n"
    )
    (probe.root / "resolved-revisions.json").write_text(
        json.dumps(revisions, indent=2) + "\n"
    )
    assert not missing, (
        f"selected upstream skills are missing: {sorted(set(missing))}; evidence: {probe.root}"
    )
    return plans, sources, roots, revisions


def test_selected_sources_and_runtime_matrix(cli_probe, source_snapshots):
    probe = cli_probe
    plans, sources, roots, revisions = source_snapshots
    for target, plan in plans.items():
        wanted = set()
        for source in plan["sources"]:
            revision = revisions.get(source["id"])
            origin = (
                source["path"]
                if source["kind"] == "local"
                else source["url"].removesuffix(".git") + "/tree/" + revision
            )
            result = probe.add(
                target + "-" + source["id"], origin, list(source["skills"]), target
            )
            rows(result, source["skills"], target, probe.destination(target), revision)
            for name, relative in source["skills"].items():
                wanted.add(name)
                assert files(probe.destination(target) / name) == files(
                    roots[source["id"]] / relative
                )
        assert {path.name for path in probe.destination(target).iterdir()} == wanted
        assert "find-skills" in wanted
    assert not (probe.workspace / ".agents").exists()
    assert not (probe.workspace / "skills-lock.json").exists()
    global_lock = json.loads(probe.lock.read_text())
    assert global_lock["version"] == 3
    for source in sources.values():
        if source["kind"] == "git":
            for name in source["skills"]:
                assert global_lock["skills"][name]["ref"] == revisions[source["id"]]


def test_pinned_search(cli_probe):
    result = cli_probe.checked(
        "find",
        ["node", str(cli_probe.cli), "find", "typescript", "--owner", "vercel-labs"],
    )
    assert re.search(r"vercel-labs/[a-zA-Z0-9_.-]+@[a-zA-Z0-9_.-]+", result.stdout), (
        result.stdout
    )
    assert "https://skills.sh/vercel-labs/" in result.stdout


def test_installer_real_cli_licenses_ownership_and_restore(
    cli_probe, source_snapshots, tmp_path, monkeypatch
):
    probe = Probe(tmp_path / "installer", cli_probe.cli)
    plans, _, roots, revisions = source_snapshots
    spec = importlib.util.spec_from_file_location(
        "agent_skills_network_installer",
        Path(__file__).resolve().parents[1] / "install-agent-skills.py",
    )
    assert spec and spec.loader
    adapter = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(adapter)
    monkeypatch.setattr(adapter, "LEGACY_CODEX_SKILLS", probe.root / "absent-legacy")

    # Preserve the installer command/protocol while reusing the package acquired
    # with --ignore-scripts. No npm or runtime configuration comes from real HOME.
    def environment(home, state):
        env = dict(probe.env)
        env["HOME"] = str(home)
        env["XDG_STATE_HOME"] = str(state)
        for variable, relative in [
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_DATA_HOME", "data"),
            ("XDG_CACHE_HOME", "cache"),
        ]:
            directory = home / "xdg" / relative
            directory.mkdir(parents=True, exist_ok=True)
            env[variable] = str(directory)
        state.mkdir(parents=True, exist_ok=True)
        return env

    monkeypatch.setattr(adapter, "environment", environment)
    counter = 0

    def run(command, *, cwd, env):
        nonlocal counter
        assert command[:3] == ["npx", "--yes", "skills@1.7.2"], command
        counter += 1
        result = probe.run(
            f"installer-{counter}", ["node", str(probe.cli), *command[3:]], cwd, env
        )
        if result.returncode:
            raise adapter.ValidationError(f"{result.stdout}\n{result.stderr}")
        return result.stdout

    monkeypatch.setattr(adapter, "run", run)
    state = Path(probe.env["XDG_STATE_HOME"])
    monkeypatch.setenv("XDG_STATE_HOME", str(state))
    local_name = "code-review"
    local_root = probe.root / "local-source"
    local_source = next(s for s in plans["claude"]["sources"] if s["kind"] == "local")
    shutil.copytree(
        Path(local_source["path"]) / local_source["skills"][local_name],
        local_root / local_name,
    )
    bounded = []
    for target, plan in plans.items():
        chosen = []
        for source in plan["sources"]:
            if source["kind"] == "local":
                assert local_name in source["skills"]
                chosen.append(
                    {
                        **source,
                        "path": str(local_root),
                        "skills": {local_name: local_name},
                    }
                )
            elif "find-skills" in source["skills"]:
                chosen.append(
                    {
                        **source,
                        "skills": {"find-skills": source["skills"]["find-skills"]},
                    }
                )
        assert len(chosen) == (2 if target == "claude" else 1)
        bounded.append({**plan, "sources": chosen})
        personal = probe.destination(target) / "personal-fixture"
        personal.mkdir(parents=True)
        (personal / "SKILL.md").write_text("user-owned fixture\n")
    external = next(s for s in bounded[0]["sources"] if s["kind"] == "git")
    fixed = {external["id"]: revisions[external["id"]]}
    foreign = {"source": "personal/fixture", "sourceType": "github"}
    global_lock = {
        "version": 3,
        "skills": {"personal-fixture": foreign},
        "dismissed": {},
    }
    adapter.write_json(probe.lock, global_lock)
    first_backup = adapter.install(bounded, probe.home, state, fixed)
    notice = (roots[external["id"]] / external["license"]).read_bytes()
    before = {}
    owners = {}
    for plan in bounded:
        target = plan["target"]
        destination = probe.destination(target)
        expected = files(roots[external["id"]] / external["skills"]["find-skills"])
        installed = files(destination / "find-skills")
        assert installed.pop("AGENT-CORE-SOURCE-LICENSE") == notice
        assert installed == expected
        if target == "claude":
            assert files(destination / local_name) == files(local_root / local_name)
        else:
            assert not (destination / local_name).exists()
        owner = adapter.read_json(adapter.owner_path(state, destination))
        assert set(owner["skills"]) == set(adapter.selected(plan))
        assert owner["skills"]["find-skills"]["revision"] == fixed[external["id"]]
        assert (
            owner["skills"]["find-skills"]["license_supplement"]
            == "AGENT-CORE-SOURCE-LICENSE"
        )
        for name, record in owner["skills"].items():
            assert record["files"] == adapter.tree(destination / name)
        before[target] = files(destination)
        owners[target] = owner
    before_locks = {probe.lock: adapter.read_json(probe.lock)}
    with (local_root / local_name / "SKILL.md").open("a") as handle:
        handle.write("\nIsolated integration fixture update.\n")
    second_backup = adapter.install(bounded, probe.home, state, fixed)
    assert files(probe.destination("claude") / local_name) == files(local_root / local_name)
    assert not (probe.destination("shared") / local_name).exists()
    adapter.restore(second_backup, probe.home, state)
    for target, expected in before.items():
        assert files(probe.destination(target)) == expected
        assert (
            adapter.read_json(adapter.owner_path(state, probe.destination(target)))
            == owners[target]
        )
    for path, expected in before_locks.items():
        assert adapter.read_json(path) == expected
        assert adapter.read_json(path)["skills"]["personal-fixture"] == foreign
    (probe.root / "installer-evidence.json").write_text(
        json.dumps(
            {
                "revisions": fixed,
                "first_backup": str(first_backup),
                "restored_backup": str(second_backup),
                "real_cli_calls": counter,
                "targets": [plan["target"] for plan in bounded],
            },
            indent=2,
        )
        + "\n"
    )


def test_cli_overwrite_partial_failure_and_recovery(cli_probe, tmp_path):
    probe = Probe(tmp_path / "fixtures", cli_probe.cli)

    def skill(source, name, content):
        directory = probe.root / source / name
        directory.mkdir(parents=True, exist_ok=True)
        (directory / "SKILL.md").write_text(
            f"---\nname: {name}\ndescription: Isolated CLI fixture.\n---\n{content}\n"
        )
        return directory

    def add(label, source, names, target="shared", json_mode=True):
        return probe.add(label, probe.root / source, names, target, json_mode)

    source = skill("source-a", "fixture-main", "original")
    (source / "stale.txt").write_text("removed by source update")
    destination = probe.destination("shared")
    rows(
        add("first", "source-a", ["fixture-main"]),
        ["fixture-main"],
        "shared",
        destination,
        None,
    )
    assert (
        probe.lock.exists() and not (probe.home / ".agents/.skill-lock.json").exists()
    )
    (destination / "fixture-main/user-extra.txt").write_text("user-created file")
    (destination / "fixture-main/SKILL.md").write_text("user edit")
    (source / "stale.txt").unlink()
    (source / "new.txt").write_text("new source file")
    rows(
        add("reinstall", "source-a", ["fixture-main"]),
        ["fixture-main"],
        "shared",
        destination,
        None,
    )
    assert files(destination / "fixture-main") == files(source)
    other = skill("source-b", "fixture-main", "different source")
    rows(
        add("collision", "source-b", ["fixture-main"]),
        ["fixture-main"],
        "shared",
        destination,
        None,
    )
    assert files(destination / "fixture-main") == files(other)
    rows(
        add("independent-copy", "source-a", ["fixture-main"], "claude"),
        ["fixture-main"],
        "claude",
        probe.destination("claude"),
        None,
    )
    assert files(destination / "fixture-main") == files(other)
    assert files(probe.destination("claude") / "fixture-main") == files(source)
    assert json.loads(probe.lock.read_text())["skills"]["fixture-main"][
        "source"
    ] == str(source.parent)
    names = ["a-good", "z-bad", "unrelated"]
    for name in names:
        skill("source-a", name, "original")
    rows(add("partial-initial", "source-a", names), names, "shared", destination, None)
    before = {name: files(destination / name) for name in names}
    before_lock = json.loads(probe.lock.read_text())
    backup = probe.root / "backup"
    for name in ["a-good", "z-bad"]:
        shutil.copytree(destination / name, backup / name)
        skill("source-a", name, "updated")
    os.mkfifo(probe.root / "source-a/z-bad/failure.fifo")
    result = add("partial-json", "source-a", ["a-good", "z-bad"])
    assert result.returncode == 1, result.stdout
    value = json.loads(result.stdout)
    assert {(row["name"], row["status"]) for row in value} == {
        ("a-good", "installed"),
        ("z-bad", "failed"),
    }
    assert "FIFO" in next(row["error"] for row in value if row["status"] == "failed")
    after_lock = json.loads(probe.lock.read_text())
    assert files(destination / "a-good") != before["a-good"]
    assert after_lock["skills"]["a-good"] != before_lock["skills"]["a-good"]
    assert after_lock["skills"]["z-bad"] == before_lock["skills"]["z-bad"]
    plain = add("partial-plain", "source-a", ["a-good", "z-bad"], json_mode=False)
    assert plain.returncode == 0 and "Failed to install" in plain.stdout
    for name in ["a-good", "z-bad"]:
        shutil.rmtree(destination / name)
        shutil.copytree(backup / name, destination / name)
    restored = json.loads(probe.lock.read_text())
    for name in ["a-good", "z-bad"]:
        restored["skills"][name] = before_lock["skills"][name]
    probe.lock.write_text(json.dumps(restored, indent=2) + "\n")
    assert all(files(destination / name) == before[name] for name in names)
    assert json.loads(probe.lock.read_text()) == before_lock
