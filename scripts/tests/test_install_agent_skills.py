from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import shutil

import pytest

SPEC = importlib.util.spec_from_file_location(
    "skill_installer", Path(__file__).resolve().parents[1] / "install-agent-skills.py"
)
installer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(installer)


@pytest.fixture
def fixture(tmp_path, monkeypatch):
    monkeypatch.setattr(installer, "LEGACY_CODEX_SKILLS", tmp_path / "etc/codex/skills")
    source = tmp_path / "source"
    for name in ("one", "two"):
        folder = source / name
        folder.mkdir(parents=True)
        (folder / "SKILL.md").write_text(
            f"---\nname: {name}\ndescription: Test skill.\n---\n\nBody.\n"
        )
        (folder / "reference.md").write_text("original\n")
    plan = {
        "target": "shared",
        "runtimes": ["codex", "pi"],
        "cli_version": "1.7.2",
        "sources": [
            {
                "id": "local",
                "kind": "local",
                "path": str(source),
                "skills": {"one": "one", "two": "two"},
            }
        ],
    }
    home = tmp_path / "home"
    home.mkdir()
    state = tmp_path / "state"
    calls = []

    def fake_cli(plan, source, revision, home, state, **kwargs):
        calls.append((plan["target"], home))
        target = installer.location(plan["target"], home)
        for name in source["skills"]:
            output = target / name
            if output.exists():
                shutil.rmtree(output)
            shutil.copytree(Path(source["path"]) / name, output)
        lock = installer.lock_paths(home, state)[0]
        value = installer.read_json(lock, {"version": 3, "skills": {}})
        for name in source["skills"]:
            value["skills"][name] = {"source": source["id"], "installation": len(calls)}
        installer.write_json(lock, value)

    monkeypatch.setattr(installer, "cli_add", fake_cli)
    return plan, home, state, calls, fake_cli


def test_install_reinstall_user_change_and_independent_targets(fixture):
    plan, home, state, calls, _ = fixture
    installer.install([plan], home, state, {})
    destination = installer.location("shared", home)
    (Path(plan["sources"][0]["path"]) / "one/reference.md").unlink()
    installer.install([plan], home, state, {})
    assert not (destination / "one/reference.md").exists()
    claude = {**plan, "target": "claude", "runtimes": ["claude"]}
    installer.install([claude], home, state, {})
    (destination / "one/SKILL.md").write_text("user edit")
    before = len(calls)
    with pytest.raises(installer.ValidationError, match="user modification"):
        installer.install([plan], home, state, {})
    assert len(calls) == before
    # Claude ownership is independent from the shared pool's changed content.
    installer.install([claude], home, state, {})


def test_hardlinked_local_source_installs_and_replays(fixture, tmp_path):
    plan, home, state, _, _ = fixture
    source = Path(plan["sources"][0]["path"]) / "one/SKILL.md"
    alias = tmp_path / "store-link"
    alias.hardlink_to(source)
    source.chmod(0o444)
    original = source.read_bytes()
    assert source.stat().st_nlink == 2

    backup = installer.install([plan], home, state, {})
    receipt = installer.read_json(backup / "operation.json")
    installer.install([plan], home, state, {}, replay=receipt)

    installed = installer.location("shared", home) / "one/SKILL.md"
    assert installed.read_bytes() == original
    assert installed.stat().st_nlink == 1
    assert not installed.samefile(source)
    assert source.read_bytes() == original
    assert source.stat().st_mode & 0o777 == 0o444


def test_hardlinked_destination_is_rejected_before_cli(fixture, tmp_path):
    plan, home, state, calls, _ = fixture
    installer.install([plan], home, state, {})
    installed = installer.location("shared", home) / "one/SKILL.md"
    (tmp_path / "other-file").hardlink_to(installed)
    before = len(calls)
    with pytest.raises(
        installer.ValidationError, match="not an ordinary unlinked file"
    ):
        installer.install([plan], home, state, {})
    assert len(calls) == before


@pytest.mark.parametrize("kind", ["symlink", "fifo"])
def test_unsafe_local_source_is_rejected_before_cli(fixture, tmp_path, kind):
    plan, home, state, calls, _ = fixture
    source = Path(plan["sources"][0]["path"]) / "one/reference.md"
    source.unlink()
    if kind == "symlink":
        other = tmp_path / "other-file"
        other.write_text("outside source")
        source.symlink_to(other)
        message = "unexpected symlink"
    else:
        os.mkfifo(source)
        message = "not an ordinary unlinked file"
    with pytest.raises(installer.ValidationError, match=message):
        installer.install([plan], home, state, {})
    assert not calls


def test_hardlinked_source_change_during_preparation_is_rejected(
    fixture, tmp_path, monkeypatch
):
    plan, home, state, calls, fake_cli = fixture
    source = Path(plan["sources"][0]["path"]) / "one/reference.md"
    alias = tmp_path / "store-link"
    alias.hardlink_to(source)

    def changing_cli(*args, **kwargs):
        fake_cli(*args, **kwargs)
        alias.write_text("changed source")

    monkeypatch.setattr(installer, "cli_add", changing_cli)
    with pytest.raises(
        installer.ValidationError, match="local source changed during preparation"
    ):
        installer.install([plan], home, state, {})
    assert all(call_home != home for _, call_home in calls)
    assert not installer.location("shared", home).exists()


def test_unowned_collision_and_symlink_parent_are_rejected_before_cli(fixture):
    plan, home, state, calls, _ = fixture
    destination = installer.location("shared", home)
    (destination / "one").mkdir(parents=True)
    (destination / "one/SKILL.md").write_text("personal skill")
    with pytest.raises(installer.ValidationError, match="unowned skill collision"):
        installer.install([plan], home, state, {})
    assert not calls
    shutil.rmtree(destination)
    destination.symlink_to(Path(plan["sources"][0]["path"]))
    with pytest.raises(installer.ValidationError, match="symlink ancestor"):
        installer.install([plan], home, state, {})
    assert not calls


def test_partial_failure_restore_preserves_unrelated_skills_and_lock_entries(
    fixture, monkeypatch
):
    plan, home, state, calls, fake_cli = fixture
    installer.install([plan], home, state, {})
    destination = installer.location("shared", home)
    original = installer.tree(destination / "one")
    unrelated = destination / "personal"
    unrelated.mkdir()
    (unrelated / "SKILL.md").write_text("personal")
    lock = installer.lock_paths(home, state)[0]
    value = installer.read_json(lock)
    value["skills"]["personal"] = {"source": "personal"}
    installer.write_json(lock, value)
    (Path(plan["sources"][0]["path"]) / "one/reference.md").write_text("new source")

    def partial(plan, source, revision, install_home, install_state, **kwargs):
        fake_cli(plan, source, revision, install_home, install_state, **kwargs)
        if install_home == home:
            raise installer.ValidationError("simulated CLI partial failure")

    monkeypatch.setattr(installer, "cli_add", partial)
    with pytest.raises(installer.ValidationError, match="Restore backup"):
        installer.install([plan], home, state, {})
    backups = state / "agent-core/skills/backups"
    failed = next(
        path
        for path in backups.iterdir()
        if installer.read_json(path / "operation.json")["status"] == "failed"
    )
    # Another CLI operation may update unrelated entries before restoration.
    value = installer.read_json(lock)
    value["skills"]["personal"]["new"] = True
    installer.write_json(lock, value)
    installer.restore(failed, home, state)
    assert installer.tree(destination / "one") == original
    assert (unrelated / "SKILL.md").read_text() == "personal"
    assert installer.read_json(lock)["skills"]["personal"]["new"] is True
    assert installer.preflight([plan], home, state)


def test_restore_refuses_edits_since_failed_operation(fixture, monkeypatch):
    plan, home, state, _, _ = fixture
    backup = installer.install([plan], home, state, {})
    (installer.location("shared", home) / "one/reference.md").write_text("user edit")
    with pytest.raises(installer.ValidationError, match="changed since backup"):
        installer.restore(backup, home, state)


def test_json_protocol_checks_all_names_paths_agents_and_revision(fixture, monkeypatch):
    plan, home, state, _, _ = fixture
    source = plan["sources"][0]
    destination = installer.location("shared", home)
    for name in source["skills"]:
        shutil.copytree(Path(source["path"]) / name, destination / name)
    rows = [
        {
            "name": name,
            "status": "installed",
            "agents": ["Codex", "Pi"],
            "mode": "copy",
            "ref": None,
            "path": str(destination / name),
        }
        for name in source["skills"]
    ]
    # The fixture replaces cli_add, so exercise the module's actual JSON adapter.
    another = importlib.util.module_from_spec(SPEC)
    SPEC.loader.exec_module(another)
    monkeypatch.setattr(another, "run", lambda *args, **kwargs: json.dumps(rows))
    another.cli_add(plan, source, None, home, state)
    rows[0]["path"] = str(home / "wrong")
    with pytest.raises(installer.ValidationError, match="incomplete CLI results"):
        another.cli_add(plan, source, None, home, state)
    rows.pop()
    with pytest.raises(installer.ValidationError, match="incomplete CLI results"):
        another.cli_add(plan, source, None, home, state)


def test_bad_sha_does_not_run_git(fixture, monkeypatch):
    plan, home, state, _, _ = fixture
    external = {
        **plan,
        "sources": [
            {
                "id": "external",
                "kind": "git",
                "url": "https://github.com/test/skills.git",
            }
        ],
    }
    monkeypatch.setattr(
        installer, "run", lambda *args, **kwargs: pytest.fail("unexpected network")
    )
    with pytest.raises(installer.ValidationError, match="invalid full SHA"):
        installer.revisions([external], {"external": "main"}, {})


def test_replay_refuses_changed_local_source_before_touching_home(fixture):
    plan, home, state, calls, _ = fixture
    backup = installer.install([plan], home, state, {})
    receipt = installer.read_json(backup / "operation.json")
    destination = installer.location("shared", home)
    original = installer.tree(destination / "one")
    (Path(plan["sources"][0]["path"]) / "one/reference.md").write_text("new source")
    with pytest.raises(
        installer.ValidationError, match="replay source content differs"
    ):
        installer.install([plan], home, state, {}, replay=receipt)
    assert installer.tree(destination / "one") == original


def test_source_change_and_stale_skill_require_review(fixture):
    plan, home, state, _, _ = fixture
    installer.install([plan], home, state, {})
    source = plan["sources"][0]
    changed = {**plan, "sources": [{**source, "id": "other"}]}
    with pytest.raises(
        installer.ValidationError, match="source change requires review"
    ):
        installer.install([changed], home, state, {})
    changed = {**plan, "sources": [{**source, "skills": {"one": "one"}}]}
    with pytest.raises(installer.ValidationError, match="stale managed skills"):
        installer.install([changed], home, state, {})


def test_restore_refuses_newer_claude_lock_without_touching_shared_files(fixture):
    plan, home, state, _, _ = fixture
    backup = installer.install([plan], home, state, {})
    original = installer.tree(installer.location("shared", home) / "one")
    installer.install(
        [{**plan, "target": "claude", "runtimes": ["claude"]}], home, state, {}
    )
    with pytest.raises(installer.ValidationError, match="lock entries changed"):
        installer.restore(backup, home, state)
    assert installer.tree(installer.location("shared", home) / "one") == original


@pytest.mark.parametrize(
    "corruption", ["destination", "owner_file", "name", "revision", "lock"]
)
def test_restore_rejects_corrupt_receipt_before_mutation(fixture, corruption):
    plan, home, state, _, _ = fixture
    backup = installer.install([plan], home, state, {})
    receipt = installer.read_json(backup / "operation.json")
    if corruption in ("destination", "owner_file"):
        receipt["targets"][0][corruption] = str(home.parent / "outside")
    elif corruption == "name":
        receipt["targets"][0]["after"]["../outside"] = None
    elif corruption == "revision":
        receipt["revisions"] = {"external": "main"}
    else:
        receipt["locks"][str(home.parent / "outside.json")] = {}
    installer.write_json(backup / "operation.json", receipt)
    destination = installer.location("shared", home)
    original = installer.tree(destination)
    with pytest.raises(installer.ValidationError):
        installer.restore(backup, home, state)
    assert installer.tree(destination) == original


def test_partial_unsafe_output_keeps_failure_receipt_and_original_backup(
    fixture, monkeypatch
):
    plan, home, state, _, fake_cli = fixture
    installer.install([plan], home, state, {})
    original = installer.tree(installer.location("shared", home) / "one")
    (Path(plan["sources"][0]["path"]) / "one/reference.md").write_text("new source")

    def unsafe(plan, source, revision, install_home, install_state, **kwargs):
        fake_cli(plan, source, revision, install_home, install_state, **kwargs)
        if install_home == home:
            output = installer.location("shared", home) / "one"
            shutil.rmtree(output)
            output.symlink_to(Path(source["path"]) / "one")
            raise installer.ValidationError("unsafe partial output")

    monkeypatch.setattr(installer, "cli_add", unsafe)
    with pytest.raises(installer.ValidationError, match="Restore backup"):
        installer.install([plan], home, state, {})
    failed = next(
        path
        for path in (state / "agent-core/skills/backups").iterdir()
        if installer.read_json(path / "operation.json")["status"] == "failed"
    )
    assert installer.read_json(failed / "operation.json")["targets"][0][
        "snapshot_errors"
    ]
    assert installer.tree(failed / "0/one") == original
    with pytest.raises(installer.ValidationError, match="manual preservation"):
        installer.restore(failed, home, state)


def test_legacy_discovery_path_requires_handoff_before_cli(fixture):
    plan, home, state, calls, _ = fixture
    legacy = home / ".pi/agent/skills"
    legacy.mkdir(parents=True)
    with pytest.raises(
        installer.ValidationError, match="legacy skill path still active"
    ):
        installer.install([plan], home, state, {})
    assert not calls


def test_multi_target_receipt_records_union_of_local_sources(fixture):
    plan, home, state, _, _ = fixture
    source = plan["sources"][0]
    claude = {
        **plan,
        "target": "claude",
        "runtimes": ["claude"],
        "sources": [{**source, "skills": {"one": "one"}}],
    }
    backup = installer.install([plan, claude], home, state, {})
    assert set(installer.read_json(backup / "operation.json")["local_hashes"]) == {
        "one",
        "two",
    }


def test_probe_acquires_sources_without_touching_legacy_home(
    fixture, monkeypatch, tmp_path
):
    plan, home, state, calls, _ = fixture
    (home / ".pi/agent/skills").mkdir(parents=True)
    monkeypatch.setattr(installer.Path, "home", lambda: home)
    monkeypatch.setattr(installer, "installation_plan", lambda target: plan)
    output = tmp_path / "probe.json"
    assert installer.main(["probe", "--target", "shared", "--output", str(output)]) == 0
    assert installer.read_json(output)["local_hashes"]
    assert not installer.location("shared", home).exists()
    assert all(call_home != home for _, call_home in calls)


def test_latest_resolves_once_per_operation_and_replay_avoids_latest(
    fixture, monkeypatch
):
    plan, home, state, _, _ = fixture
    source = {
        "id": "external",
        "kind": "git",
        "url": "https://github.com/test/skills.git",
    }
    plans = [
        {**plan, "target": target, "sources": [source]}
        for target in ("shared", "claude")
    ]
    calls = []

    def remote_head(*args, **kwargs):
        calls.append(args)
        return f"ref: refs/heads/main\tHEAD\n{str(len(calls)) * 40}\tHEAD\n"

    monkeypatch.setattr(installer, "run", remote_head)
    first = installer.revisions(plans, {}, {})
    second = installer.revisions(plans, {}, {})
    assert first == {"external": "1" * 40}
    assert second == {"external": "2" * 40}
    assert installer.revisions(plans, first, {}) == first
    assert len(calls) == 2


def test_license_acquisition_failure_preserves_installed_files(fixture, monkeypatch):
    plan, home, state, calls, _ = fixture
    installer.install([plan], home, state, {})
    destination = installer.location("shared", home)
    before = installer.tree(destination)
    previous_calls = len(calls)
    source = {
        **plan["sources"][0],
        "kind": "git",
        "url": "https://github.com/test/skills.git",
        "license": "LICENSE",
    }

    def missing(*args, **kwargs):
        raise installer.ValidationError("missing license")

    monkeypatch.setattr(installer, "license_bytes", missing)
    with pytest.raises(installer.ValidationError, match="missing license"):
        installer.install(
            [{**plan, "sources": [source]}], home, state, {"local": "1" * 40}
        )
    assert len(calls) == previous_calls
    assert installer.tree(destination) == before


def test_unchanged_install_skips_preparation_and_backup(fixture, monkeypatch):
    plan, home, state, calls, _ = fixture
    backup = installer.install([plan], home, state, {})
    count = len(calls)
    monkeypatch.setattr(
        installer, "prepare", lambda *a, **kw: pytest.fail("prepared unchanged install")
    )
    assert installer.install([plan], home, state, {}) is None
    assert len(calls) == count
    assert list(backup.parent.iterdir()) == [backup]


def test_shared_and_claude_targets_skip_together(fixture):
    plan, home, state, calls, _ = fixture
    claude = {**plan, "target": "claude", "runtimes": ["claude"]}
    installer.install([plan, claude], home, state, {})
    count = len(calls)
    assert installer.install([plan, claude], home, state, {}) is None
    assert len(calls) == count


@pytest.mark.parametrize(
    "change",
    [
        "content",
        "mode",
        "cli",
        "lock",
        "missing_lock",
        "old_owner",
        "owner_metadata",
        "failed_receipt",
    ],
)
def test_changed_install_inputs_require_preparation(fixture, change):
    plan, home, state, calls, _ = fixture
    backup = installer.install([plan], home, state, {})
    count = len(calls)
    source = Path(plan["sources"][0]["path"]) / "one/reference.md"
    if change == "content":
        source.write_text("updated")
    elif change == "mode":
        source.chmod(0o755)
    elif change == "cli":
        plan = {**plan, "cli_version": "new-version"}
    elif change in ("lock", "missing_lock"):
        path = installer.lock_paths(home, state)[0]
        if change == "missing_lock":
            path.unlink()
        else:
            value = installer.read_json(path)
            value["skills"]["one"] = {"changed": True}
            installer.write_json(path, value)
    elif change in ("old_owner", "owner_metadata"):
        path = installer.owner_path(state, installer.location("shared", home))
        value = installer.read_json(path)
        if change == "old_owner":
            value.pop("installation")
        else:
            value["skills"]["one"]["revision"] = "changed"
        installer.write_json(path, value)
    else:
        receipt = installer.read_json(backup / "operation.json")
        receipt["status"] = "failed"
        installer.write_json(backup / "operation.json", receipt)
    assert installer.install([plan], home, state, {}) is not None
    assert len(calls) == count + 2


@pytest.mark.parametrize("change", ["missing", "modified", "unowned"])
def test_cached_install_preserves_ownership_checks(fixture, change):
    plan, home, state, calls, _ = fixture
    installer.install([plan], home, state, {})
    count = len(calls)
    destination = installer.location("shared", home)
    if change == "missing":
        shutil.rmtree(destination / "one")
    elif change == "modified":
        (destination / "one/reference.md").write_text("user edit")
    else:
        installer.owner_path(state, destination).unlink()
    with pytest.raises(
        installer.ValidationError, match="user modification|unowned skill collision"
    ):
        installer.install([plan], home, state, {})
    assert len(calls) == count


def test_failed_install_retries_instead_of_caching(fixture, monkeypatch):
    plan, home, state, calls, fake_cli = fixture
    installer.install([plan], home, state, {})
    (Path(plan["sources"][0]["path"]) / "one/reference.md").write_text("new source")

    def failure(plan, source, revision, install_home, install_state, **kwargs):
        if install_home == home:
            raise installer.ValidationError("retry required")
        fake_cli(plan, source, revision, install_home, install_state, **kwargs)

    monkeypatch.setattr(installer, "cli_add", failure)
    with pytest.raises(installer.ValidationError, match="retry required"):
        installer.install([plan], home, state, {})
    count = len(calls)
    monkeypatch.setattr(installer, "cli_add", fake_cli)
    assert installer.install([plan], home, state, {}) is not None
    assert len(calls) == count + 2


def test_remote_head_checked_once_even_when_install_is_unchanged(fixture, monkeypatch):
    plan, home, state, calls, _ = fixture
    source = {
        **plan["sources"][0],
        "kind": "git",
        "url": "https://github.com/test/skills.git",
        "license": "LICENSE",
    }
    plan = {**plan, "sources": [source]}
    revision = ["1" * 40]
    remote_calls = []
    license_calls = []

    def remote(*args, **kwargs):
        remote_calls.append(args)
        return f"ref: refs/heads/main\tHEAD\n{revision[0]}\tHEAD\n"

    def license(*args):
        license_calls.append(args)
        return b"license"

    monkeypatch.setattr(installer, "run", remote)
    monkeypatch.setattr(installer, "license_bytes", license)
    installer.install([plan], home, state, {})
    count = len(calls)
    assert installer.install([plan], home, state, {}) is None
    assert len(remote_calls) == 2
    assert len(license_calls) == 1
    assert len(calls) == count
    revision[0] = "2" * 40
    assert installer.install([plan], home, state, {}) is not None
    assert len(remote_calls) == 3
    assert len(license_calls) == 2
    assert len(calls) == count + 2
