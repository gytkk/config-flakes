from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time

import pytest

SPEC = importlib.util.spec_from_file_location(
    "skill_installer", Path(__file__).resolve().parents[1] / "install-agent-skills.py"
)
installer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(installer)


@pytest.mark.parametrize("launcher_exits", [False, True])
@pytest.mark.parametrize("failure", ["timeout", "interrupt", "io"])
def test_command_failure_stops_descendant_writes(
    tmp_path, monkeypatch, launcher_exits, failure
):
    marker = tmp_path / "late-write"
    ready = tmp_path / "child-pid"
    child = (
        "import os, signal, time; from pathlib import Path; "
        "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
        f"Path({str(ready)!r}).write_text(str(os.getpid())); "
        f"time.sleep(0.6); Path({str(marker)!r}).write_text('late')"
    )
    launcher = (
        "import subprocess, sys, time; from pathlib import Path; "
        f"subprocess.Popen([sys.executable, '-c', {child!r}]); "
        f"ready = Path({str(ready)!r})\n"
        "while not ready.exists(): time.sleep(0.005)\n"
        + ("sys.exit(7)" if launcher_exits else "time.sleep(3)")
    )
    communicate = subprocess.Popen.communicate

    def short_timeout(process, *args, **kwargs):
        if kwargs.get("timeout") == 300:
            kwargs["timeout"] = 0.2
        try:
            return communicate(process, *args, **kwargs)
        except subprocess.TimeoutExpired:
            if failure == "interrupt":
                raise KeyboardInterrupt("simulated interruption")
            if failure == "io":
                raise OSError("simulated pipe failure")
            raise

    monkeypatch.setattr(subprocess.Popen, "communicate", short_timeout)
    try:
        error = (
            KeyboardInterrupt if failure == "interrupt" else installer.ValidationError
        )
        with pytest.raises(error):
            installer.run(
                [sys.executable, "-c", launcher], cwd=tmp_path, env=os.environ
            )
        assert ready.exists(), "launcher did not start the synthetic child"
        time.sleep(0.7)
        assert not marker.exists(), "descendant wrote after command timeout"
    finally:
        if ready.exists():
            try:
                os.kill(int(ready.read_text()), signal.SIGKILL)
            except ProcessLookupError:
                pass


def test_command_output_and_failure_messages(tmp_path):
    assert (
        installer.run(
            [sys.executable, "-c", "print('captured')"], cwd=tmp_path, env=os.environ
        )
        == "captured\n"
    )
    with pytest.raises(installer.ValidationError, match="command exited 7") as caught:
        installer.run(
            [
                sys.executable,
                "-c",
                "import sys; print('out'); print('err', file=sys.stderr); sys.exit(7)",
            ],
            cwd=tmp_path,
            env=os.environ,
        )
    assert "\nout\n\nerr\n" in str(caught.value)
    with pytest.raises(installer.ValidationError, match="command failed"):
        installer.run([str(tmp_path / "missing-command")], cwd=tmp_path, env=os.environ)


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
        "runtimes": ["codex", "pi", "omp"],
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
    expected = installer.tree(destination / "one")
    (destination / "one/SKILL.md").write_text("user edit")
    # Claude ownership is independent from the shared pool's changed content.
    assert installer.install([claude], home, state, {}) is None
    backup = installer.install([plan], home, state, {})
    assert installer.tree(destination / "one") == expected
    assert (backup / "0/one/SKILL.md").read_text() == "user edit"


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


def test_unowned_collision_is_replaced_and_symlink_parent_rejected(fixture):
    plan, home, state, calls, _ = fixture
    destination = installer.location("shared", home)
    (destination / "one").mkdir(parents=True)
    (destination / "one/SKILL.md").write_text("personal skill")
    backup = installer.install([plan], home, state, {})
    assert (destination / "one/reference.md").read_text() == "original\n"
    assert (backup / "0/one/SKILL.md").read_text() == "personal skill"
    calls.clear()
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


@pytest.mark.parametrize("corruption", ["owner", "lock", "both"])
@pytest.mark.parametrize("changed", [False, True])
def test_metadata_corruption_keeps_original_failure_and_backup(
    fixture, monkeypatch, corruption, changed
):
    plan, home, state, _, fake_cli = fixture
    installer.install([plan], home, state, {})
    destination = installer.location("shared", home)
    original = installer.tree(destination / "one")
    owner_file = installer.owner_path(state, destination)
    previous_owner = owner_file.read_bytes()
    for path in installer.lock_paths(home, state):
        value = installer.read_json(path)
        value["skills"]["personal"] = {"source": "personal"}
        value["unrelated_metadata"] = {"keep": True}
        installer.write_json(path, value)
    previous_locks = {
        path: path.read_bytes() for path in installer.lock_paths(home, state)
    }
    (Path(plan["sources"][0]["path"]) / "one/reference.md").write_text("new source")
    damaged = []
    if corruption in ("owner", "both"):
        damaged.append(installer.owner_path(state, destination))
    if corruption in ("lock", "both"):
        damaged.extend(installer.lock_paths(home, state))

    def partial(plan, source, revision, install_home, install_state, **kwargs):
        fake_cli(plan, source, revision, install_home, install_state, **kwargs)
        if install_home == home:
            for path in damaged:
                path.write_text("{damaged")
            raise installer.ValidationError("original CLI failure")

    monkeypatch.setattr(installer, "cli_add", partial)
    with pytest.raises(
        installer.ValidationError, match="original CLI failure"
    ) as caught:
        installer.install([plan], home, state, {})
    assert "Restore backup:" in str(caught.value)
    assert str(caught.value.__cause__) == "original CLI failure"
    failed = next(
        path
        for path in (state / "agent-core/skills/backups").iterdir()
        if installer.read_json(path / "operation.json")["status"] == "failed"
    )
    receipt = installer.read_json(failed / "operation.json")
    assert receipt["error"] == "original CLI failure"
    for path in damaged:
        assert any(str(path) in error for error in receipt["snapshot_errors"])
    assert installer.tree(failed / "0/one") == original
    current = installer.tree(destination)
    if changed:
        damaged[-1].write_text("{later change")
        with pytest.raises(installer.ValidationError, match="changed since backup"):
            installer.restore(failed, home, state)
        assert installer.tree(destination) == current
        assert damaged[-1].read_text() == "{later change"
    else:
        installer.restore(failed, home, state)
        assert installer.tree(destination / "one") == original
        assert owner_file.read_bytes() == previous_owner
        assert all(
            path.read_bytes() == content for path, content in previous_locks.items()
        )
        assert installer.preflight([plan], home, state)


def test_unreadable_failed_metadata_keeps_receipt_and_refuses_restore(
    fixture, monkeypatch
):
    plan, home, state, _, fake_cli = fixture
    installer.install([plan], home, state, {})
    destination = installer.location("shared", home)
    owner = installer.owner_path(state, destination)
    original = installer.tree(destination / "one")
    (Path(plan["sources"][0]["path"]) / "one/reference.md").write_text("new source")
    read_bytes = Path.read_bytes

    def unreadable(path):
        if path == owner:
            raise OSError("cannot snapshot owner bytes")
        return read_bytes(path)

    def partial(plan, source, revision, install_home, install_state, **kwargs):
        fake_cli(plan, source, revision, install_home, install_state, **kwargs)
        if install_home == home:
            owner.write_text("{damaged")
            monkeypatch.setattr(Path, "read_bytes", unreadable)
            raise installer.ValidationError("original CLI failure")

    monkeypatch.setattr(installer, "cli_add", partial)
    with pytest.raises(installer.ValidationError, match="original CLI failure"):
        installer.install([plan], home, state, {})
    failed = next(
        path
        for path in (state / "agent-core/skills/backups").iterdir()
        if installer.read_json(path / "operation.json")["status"] == "failed"
    )
    receipt = installer.read_json(failed / "operation.json")
    assert receipt["snapshot_incomplete"] is True
    assert any(
        "cannot snapshot owner bytes" in error for error in receipt["snapshot_errors"]
    )
    assert installer.tree(failed / "0/one") == original
    current = installer.tree(destination)
    with pytest.raises(installer.ValidationError, match="manual preservation"):
        installer.restore(failed, home, state)
    assert installer.tree(destination) == current


def test_first_install_corrupt_lock_restores_absence(fixture, monkeypatch):
    plan, home, state, _, fake_cli = fixture
    lock = installer.lock_paths(home, state)[0]
    assert not lock.exists()

    def partial(plan, source, revision, install_home, install_state, **kwargs):
        fake_cli(plan, source, revision, install_home, install_state, **kwargs)
        if install_home == home:
            lock.write_text("{damaged")
            raise installer.ValidationError("original CLI failure")

    monkeypatch.setattr(installer, "cli_add", partial)
    with pytest.raises(installer.ValidationError, match="original CLI failure"):
        installer.install([plan], home, state, {})
    backup = next((state / "agent-core/skills/backups").iterdir())
    installer.restore(backup, home, state)
    assert not lock.exists()
    destination = installer.location("shared", home)
    assert not installer.owner_path(state, destination).exists()
    assert all(not (destination / name).exists() for name in ("one", "two"))


def test_failed_receipt_write_preserves_original_failure(fixture, monkeypatch):
    plan, home, state, _, fake_cli = fixture
    write_json = installer.write_json

    def fail_receipt(path, value):
        if path.name == "operation.json" and value["status"] == "failed":
            raise OSError("receipt disk failure")
        write_json(path, value)

    def partial(plan, source, revision, install_home, install_state, **kwargs):
        fake_cli(plan, source, revision, install_home, install_state, **kwargs)
        if install_home == home:
            raise installer.ValidationError("original CLI failure")

    monkeypatch.setattr(installer, "write_json", fail_receipt)
    monkeypatch.setattr(installer, "cli_add", partial)
    with pytest.raises(
        installer.ValidationError, match="original CLI failure"
    ) as caught:
        installer.install([plan], home, state, {})
    backup = next((state / "agent-core/skills/backups").iterdir())
    assert "receipt disk failure" in str(caught.value)
    assert str(backup) in str(caught.value)
    assert str(caught.value.__cause__) == "original CLI failure"
    assert installer.read_json(backup / "operation.json")["status"] == "prepared"


@pytest.mark.parametrize("snapshot", ["owner", "files"])
def test_snapshot_failure_after_successful_mutation_marks_failed(
    fixture, monkeypatch, snapshot
):
    plan, home, state, _, _ = fixture
    destination = installer.location("shared", home)
    owner = installer.owner_path(state, destination)
    write_json = installer.write_json
    tree = installer.tree
    snapshot_phase = False

    def finish_owner(path, value):
        nonlocal snapshot_phase
        write_json(path, value)
        if path == owner and "installation_receipt" in value:
            snapshot_phase = True
            if snapshot == "owner":
                owner.write_text("{damaged")

    def fail_tree(path, **kwargs):
        if snapshot_phase and snapshot == "files" and path == destination / "one":
            raise OSError("file snapshot failure")
        return tree(path, **kwargs)

    monkeypatch.setattr(installer, "write_json", finish_owner)
    monkeypatch.setattr(installer, "tree", fail_tree)
    with pytest.raises(installer.ValidationError, match="Restore backup"):
        installer.install([plan], home, state, {})
    backup = next((state / "agent-core/skills/backups").iterdir())
    receipt = installer.read_json(backup / "operation.json")
    assert receipt["status"] == "failed"
    assert receipt["error"]
    assert (destination / "one/SKILL.md").exists()
    if snapshot == "files":
        with pytest.raises(installer.ValidationError, match="manual preservation"):
            installer.restore(backup, home, state)
        assert (destination / "one/SKILL.md").exists()
    else:
        installer.restore(backup, home, state)
        assert not (destination / "one").exists()


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


def test_source_change_reinstalls_and_stale_skill_is_removed(fixture):
    plan, home, state, _, _ = fixture
    installer.install([plan], home, state, {})
    destination = installer.location("shared", home)
    owner_file = installer.owner_path(state, destination)
    source = plan["sources"][0]
    changed = {**plan, "sources": [{**source, "id": "other"}]}
    installer.install([changed], home, state, {})
    assert installer.read_json(owner_file)["skills"]["one"]["source"] == "other"
    changed = {**plan, "sources": [{**source, "skills": {"one": "one"}}]}
    original = installer.tree(destination / "two")
    backup = installer.install([changed], home, state, {})
    assert not (destination / "two").exists()
    assert set(installer.read_json(owner_file)["skills"]) == {"one"}
    installer.restore(backup, home, state)
    assert installer.tree(destination / "two") == original


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


@pytest.mark.parametrize("failures, succeeds", [(4, True), (5, False)])
def test_remote_head_retries_transient_failures(
    fixture, monkeypatch, failures, succeeds
):
    plan, _, _, _, _ = fixture
    source = {"id": "external", "kind": "git", "url": "https://github.com/test/skills.git"}
    calls = []
    sleeps = []

    def flaky(*args, **kwargs):
        calls.append(args)
        if len(calls) <= failures:
            raise installer.ValidationError("Could not resolve host: github.com")
        return f"ref: refs/heads/main\tHEAD\n{'1' * 40}\tHEAD\n"

    monkeypatch.setattr(installer, "run", flaky)
    monkeypatch.setattr(installer.time, "sleep", sleeps.append)
    plans = [{**plan, "sources": [source]}]
    if succeeds:
        assert installer.revisions(plans, {}, {}) == {"external": "1" * 40}
    else:
        with pytest.raises(installer.ValidationError, match="resolve host"):
            installer.revisions(plans, {}, {})
    assert len(calls) == min(failures + 1, 5)
    assert sleeps == [3] * 4


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
def test_cached_install_repairs_drift_from_manifest(fixture, change):
    plan, home, state, calls, _ = fixture
    installer.install([plan], home, state, {})
    destination = installer.location("shared", home)
    expected = installer.tree(destination / "one")
    if change == "missing":
        shutil.rmtree(destination / "one")
    elif change == "modified":
        (destination / "one/reference.md").write_text("user edit")
    else:
        installer.owner_path(state, destination).unlink()
    assert installer.install([plan], home, state, {}) is not None
    assert installer.tree(destination / "one") == expected
    assert installer.install([plan], home, state, {}) is None


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
