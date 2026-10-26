from __future__ import annotations

import copy
import json
import socket
import subprocess
from pathlib import Path, PurePosixPath

import pytest

from agent_core import core, skill_sources
from agent_core.cli import main


@pytest.mark.parametrize(
    ("target", "count"), [("shared", 22), ("claude", 19)]
)
def test_plan_selections_apply_approved_addition_and_removal(
    target: str, count: int
) -> None:
    manifest = core._load_manifest()
    plan = skill_sources.installation_plan(target)
    selected = {name for source in plan["sources"] for name in source["skills"]}
    legacy = {
        name
        for runtime in plan["runtimes"]
        for name in manifest["runtimes"][runtime]["skills"]
    }
    additions = {"find-skills", "korean"}
    if target == "shared":
        additions.add("impeccable")
    assert selected == (legacy | additions) - {
        "resolving-merge-conflicts"
    }
    assert len(selected) == count
    assert plan["sources"][0]["skills"]["parallel-research-merge"] == (
        "parallel-research-merge"
    )
    assert plan["sources"][0]["skills"]["diagnosing-bugs"] == "diagnosing-bugs"
    assert all(
        "diagnosing-bugs" not in source["skills"] for source in plan["sources"][1:]
    )
    assert plan["cli_version"] == "1.7.2"
    assert plan["target"] == target
    assert plan["runtimes"] == skill_sources.TARGET_RUNTIMES[target]
    sources = ["local", "matt", "vercel"]
    if target == "shared":
        sources.append("impeccable")
        assert plan["sources"][-1]["skills"] == {
            "impeccable": ".agents/skills/impeccable"
        }
    assert [source["id"] for source in plan["sources"]] == sources
    assert (
        Path(plan["sources"][0]["path"]) == (core.resource_root() / "skills").resolve()
    )
    for source in plan["sources"][1:]:
        assert source["update"] == "latest-default-branch"
        assert source["license"] == "LICENSE"


def test_plan_is_offline_and_does_not_consult_home(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def forbidden(*args: object, **kwargs: object) -> None:
        pytest.fail("pure plan consulted external state")

    monkeypatch.setattr(Path, "home", forbidden)
    monkeypatch.setattr(socket, "create_connection", forbidden)
    monkeypatch.setattr(subprocess, "run", forbidden)
    assert skill_sources.installation_plan("shared") == skill_sources.installation_plan(
        "shared"
    )


def test_external_paths_need_no_local_directory() -> None:
    manifest = core._load_manifest()
    manifest["skill_sources"]["matt"]["skills"]["wizard"] = (
        "not-present/anywhere/wizard"
    )
    skill_sources.validate_sources(manifest)


@pytest.mark.parametrize(
    ("table", "key", "value", "message"),
    [
        (("skill_cli",), "version", "latest", "must be pinned"),
        (("skill_cli",), "unexpected", True, "unknown or missing keys"),
        (("skill_sources", "matt"), "update", "main", "update policy"),
        (
            ("skill_sources", "matt"),
            "url",
            "http://github.com/mattpocock/skills.git",
            "URL",
        ),
        (
            ("skill_sources", "matt"),
            "url",
            "https://token@github.com/mattpocock/skills.git",
            "URL",
        ),
        (
            ("skill_sources", "matt"),
            "url",
            "https://github.com/mattpocock/skills.git#main",
            "URL",
        ),
        (("skill_sources", "matt"), "license", "../LICENSE", "unsafe"),
        (("skill_sources", "local"), "path", "/tmp/skills", "unsafe"),
        (("skill_sources", "local"), "path", "skills/./", "unsafe"),
        (
            ("skill_sources", "matt", "skills"),
            "wizard",
            "skills/../../wizard",
            "unsafe",
        ),
        (
            ("skill_sources", "matt", "skills"),
            "bad\\name",
            "skills/bad",
            "invalid skill",
        ),
        (
            ("skill_sources", "matt", "skills"),
            "code-review",
            "skills/code-review",
            "duplicate skill name",
        ),
        (
            ("skill_targets", "shared"),
            "skills",
            ["find-skills", "find-skills"],
            "duplicate skill",
        ),
        (("skill_targets", "shared"), "skills", ["missing"], "unknown skill"),
        (("skill_targets", "shared"), "runtimes", ["codex"], "invalid runtimes"),
    ],
)
def test_invalid_declarations_are_rejected(
    table: tuple[str, ...], key: str, value: object, message: str
) -> None:
    manifest = copy.deepcopy(core._load_manifest())
    destination = manifest
    for part in table:
        destination = destination[part]
    destination[key] = value
    with pytest.raises(core.ValidationError, match=message):
        skill_sources.validate_sources(manifest)


def test_local_skill_structure_is_validated(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    manifest = core._load_manifest()
    local = tmp_path / "skills" / "code-review"
    local.mkdir(parents=True)
    manifest["skill_sources"]["local"]["skills"] = {"code-review": "code-review"}
    monkeypatch.setattr(core, "resource_root", lambda: tmp_path)
    with pytest.raises(core.ValidationError, match="missing SKILL.md"):
        skill_sources.validate_sources(manifest)
    (local / "SKILL.md").write_text(
        "---\nname: wrong\ndescription: Test.\n---\nBody.\n"
    )
    with pytest.raises(core.ValidationError, match="mismatch"):
        skill_sources.validate_sources(manifest)


def test_cli_plan_matches_api(capsys: pytest.CaptureFixture[str]) -> None:
    assert main(["skill-plan", "--target", "claude"]) == 0
    assert json.loads(capsys.readouterr().out) == skill_sources.installation_plan(
        "claude"
    )
    with pytest.raises(core.ValidationError, match="unknown skill target"):
        skill_sources.installation_plan("unknown")


def test_document_hashes_detect_instruction_drift(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    assert skill_sources.check_document_hashes() == []
    original = skill_sources.materialize

    def drift(runtime: str) -> dict:
        files = dict(original(runtime))
        if runtime == "codex":
            files[PurePosixPath("AGENTS.md")] += b"drift"
        return files

    monkeypatch.setattr(skill_sources, "materialize", drift)
    assert skill_sources.check_document_hashes() == [
        "codex/AGENTS.md: instruction document hash differs"
    ]
