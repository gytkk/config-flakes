#!/usr/bin/env python3
"""Explicit runtime installation adapter for the agent-core skill plan."""

from __future__ import annotations

import argparse
from contextlib import contextmanager
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import urllib.request

from agent_core.core import ValidationError, _skill_body
from agent_core.skill_sources import installation_plan
from agent_core.installation_receipt import validate_receipt as validate_installation_receipt

SHA = re.compile(r"[0-9a-f]{40}\Z")
LEGACY_CODEX_SKILLS = Path("/etc/codex/skills")


def read_json(path, default=None):
    if not path.exists():
        return default
    try:
        return json.loads(path.read_text())
    except (ValueError, OSError) as error:
        raise ValidationError(f"invalid JSON at {path}: {error}") from error


def write_json(path, value):
    write_bytes(path, (json.dumps(value, indent=2, sort_keys=True) + "\n").encode())


def write_bytes(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(".tmp")
    temporary.write_bytes(value)
    temporary.replace(path)


def tree(path, *, allow_hardlinks=False):
    """Fingerprint ordinary installation files, including modes and directories."""
    if path.is_symlink():
        raise ValidationError(f"symlink requires ownership handoff: {path}")
    if not path.exists():
        return None
    if not path.is_dir():
        raise ValidationError(f"expected skill directory: {path}")
    result = {}
    for entry in sorted(path.rglob("*")):
        relative = entry.relative_to(path).as_posix()
        if entry.is_symlink():
            raise ValidationError(f"unexpected symlink: {entry}")
        if entry.is_dir():
            result[relative] = "directory"
        elif entry.is_file() and (allow_hardlinks or entry.stat().st_nlink == 1):
            digest = hashlib.sha256(entry.read_bytes()).hexdigest()
            result[relative] = f"{entry.stat().st_mode & 0o777:o}:{digest}"
        else:
            raise ValidationError(f"not an ordinary unlinked file: {entry}")
    return result


def environment(home, state):
    # Git's exact-SHA fallback in skills 1.7.2 parses English error messages.
    env = {
        key: os.environ[key]
        for key in ("PATH", "TMPDIR", "SSL_CERT_FILE")
        if key in os.environ
    }
    env.update(
        HOME=str(home),
        XDG_STATE_HOME=str(state),
        DISABLE_TELEMETRY="1",
        LC_ALL="C",
        LANG="C",
        GIT_CONFIG_NOSYSTEM="1",
        GIT_CONFIG_GLOBAL=os.devnull,
        GIT_TERMINAL_PROMPT="0",
        NPM_CONFIG_USERCONFIG=os.devnull,
    )
    return env


def run(command, *, cwd, env):
    try:
        process = subprocess.Popen(
            command,
            cwd=cwd,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
        )
        try:
            stdout, stderr = process.communicate(timeout=300)
        finally:
            try:
                # Descendants may keep pipes open after the launcher exits.
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait(timeout=5)
            finally:
                process.stdout.close()
                process.stderr.close()
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ValidationError(f"command failed: {command[0]}: {error}") from error
    if process.returncode:
        raise ValidationError(
            f"command exited {process.returncode}: {' '.join(command)}\n{stdout}\n{stderr}"
        )
    return stdout


def location(target, home):
    if target == "shared":
        return home / ".agents/skills"
    if target == "claude":
        return home / ".claude/skills"
    raise ValidationError(f"unsupported installation target: {target}")


def checked_parent(path):
    for parent in [path, *path.parents]:
        if parent.is_symlink():
            raise ValidationError(f"installation path has a symlink ancestor: {parent}")


def owner_path(state, destination):
    key = hashlib.sha256(str(destination).encode()).hexdigest()
    return state / "agent-core/skills" / f"{key}.json"


def selected(plan):
    return {name: source for source in plan["sources"] for name in source["skills"]}


def cli_add(plan, source, revision, home, state, *, isolated=False):
    destination = location(plan["target"], home)
    working = home
    working.mkdir(parents=True, exist_ok=True)
    origin = (
        source["path"]
        if source["kind"] == "local"
        else source["url"].removesuffix(".git") + "/tree/" + revision
    )
    command = [
        "npx",
        "--yes",
        "skills@" + plan["cli_version"],
        "add",
        origin,
        "--skill",
        *source["skills"],
        "--agent",
    ]
    command += (
        ["codex", "pi"]
        if plan["target"] == "shared"
        else ["claude-code"]
    )
    if plan["target"] != "shared":
        command += ["--copy"]
    command += ["--global"]
    command += ["--yes", "--json"]
    env = environment(home, state)
    if not isolated and "XDG_STATE_HOME" not in os.environ:
        env.pop("XDG_STATE_HOME")
    response = run(command, cwd=working, env=env)
    try:
        rows = json.loads(response)
    except ValueError as error:
        raise ValidationError(
            f"{source['id']}:{plan['target']}: invalid CLI JSON: {response}"
        ) from error
    if not isinstance(rows, list):
        raise ValidationError(
            f"{source['id']}:{plan['target']}: expected CLI result array"
        )
    expected = set(source["skills"])
    agents = (
        ["Codex", "Pi"]
        if plan["target"] == "shared"
        else ["Claude Code"]
    )
    if any(not isinstance(row, dict) for row in rows):
        raise ValidationError(
            f"{source['id']}:{plan['target']}: invalid CLI result row"
        )
    actual = {row.get("name") for row in rows if row.get("status") == "installed"}
    if (
        actual != expected
        or len(rows) != len(expected)
        or any(
            row.get("agents") != agents
            or row.get("mode") != "copy"
            or row.get("ref") != revision
            or row.get("path") != str(destination / row["name"])
            for row in rows
        )
    ):
        raise ValidationError(
            f"{source['id']}:{plan['target']}: incomplete CLI results: {response}"
        )
    for name in source["skills"]:
        path = destination / name
        _skill_body(path / "SKILL.md", name, b"")
        tree(path)
    if source["kind"] == "git":
        source_lock = (
            state / "skills/.skill-lock.json"
            if "XDG_STATE_HOME" in env
            else home / ".agents/.skill-lock.json"
        )
        entries = lock_entries(source_lock, source["skills"])
        for name, relative in source["skills"].items():
            entry = entries[name]
            if (
                not isinstance(entry, dict)
                or entry.get("ref") != revision
                or entry.get("skillPath") != relative + "/SKILL.md"
            ):
                raise ValidationError(
                    f"{source['id']}:{name}: source SHA/path differs in CLI lock"
                )


def revisions(plans, overrides, env):
    resolved = {}
    sources = {source["id"]: source for plan in plans for source in plan["sources"]}
    for identifier, source in sources.items():
        if source["kind"] == "local":
            continue
        revision = overrides.get(identifier)
        if revision is None:
            # Read-only, so retry through the brief DNS outage while a NixOS switch restarts networking.
            for attempt in range(5):
                try:
                    output = run(
                        ["git", "ls-remote", "--symref", source["url"], "HEAD"],
                        cwd=Path.cwd(),
                        env=env,
                    )
                    break
                except ValidationError:
                    if attempt == 4:
                        raise
                    time.sleep(3)
            matches = re.findall(r"^([0-9a-f]{40})\s+HEAD$", output, re.MULTILINE)
            if len(matches) != 1 or not re.search(
                r"^ref: refs/heads/[^\s]+\s+HEAD$", output, re.MULTILINE
            ):
                raise ValidationError(f"cannot resolve default branch: {identifier}")
            revision = matches[0]
        if not isinstance(revision, str) or not SHA.fullmatch(revision):
            raise ValidationError(f"invalid full SHA: {identifier}={revision}")
        resolved[identifier] = revision
    if set(overrides) - set(resolved):
        raise ValidationError("revision specified for an unknown or local source")
    return resolved


def license_bytes(source, revision):
    match = re.fullmatch(r"https://github.com/([\w.-]+/[\w.-]+)\.git", source["url"])
    if not match:
        raise ValidationError(f"unsupported license origin: {source['url']}")
    url = f"https://raw.githubusercontent.com/{match[1]}/{revision}/{source['license']}"
    try:
        with urllib.request.urlopen(url, timeout=60) as response:
            content = response.read()
    except OSError as error:
        raise ValidationError(f"missing license at {url}: {error}") from error
    if not content.strip():
        raise ValidationError(f"empty license at {url}")
    return content


def add_licenses(plan, destination, notices):
    supplements = {}
    for name, source in selected(plan).items():
        if source["kind"] == "git":
            # Preserve upstream SKILL.md and license files byte for byte.
            notice = destination / name / "AGENT-CORE-SOURCE-LICENSE"
            if notice.exists():
                raise ValidationError(
                    f"reserved license supplement already exists: {notice}"
                )
            notice.write_bytes(notices[source["id"]])
            supplements[name] = notice.name
    return supplements


def lock_paths(home, state):
    paths = {
        state / "skills/.skill-lock.json"
        if "XDG_STATE_HOME" in os.environ
        else home / ".agents/.skill-lock.json"
    }
    return sorted(paths)


def lock_entries(path, names):
    value = read_json(path, {})
    if not isinstance(value, dict) or not isinstance(value.get("skills", {}), dict):
        raise ValidationError(f"invalid CLI lock: {path}")
    return {name: value.get("skills", {}).get(name) for name in names}


def restore_lock(path, snapshot):
    if "after_bytes" in snapshot:
        if snapshot["existed"]:
            write_bytes(path, bytes.fromhex(snapshot["before_bytes"]))
        else:
            path.unlink(missing_ok=True)
        return
    value = read_json(path, {})
    skills = value.setdefault("skills", {})
    for name, entry in snapshot["before"].items():
        if entry is None:
            skills.pop(name, None)
        else:
            skills[name] = entry
    if not skills and not snapshot["existed"]:
        path.unlink(missing_ok=True)
    else:
        write_json(path, value)


@contextmanager
def exclusive(state):
    path = state / "agent-core/skills/installer.lock"
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a") as handle:
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise ValidationError(
                "another agent-core skill installation is running"
            ) from error
        yield


def preflight(plans, home, state):
    items = []
    for plan in plans:
        if plan["target"] == "shared":
            for legacy in (LEGACY_CODEX_SKILLS, home / ".pi/agent/skills"):
                if legacy.exists() or legacy.is_symlink():
                    raise ValidationError(
                        f"legacy skill path still active; complete ownership handoff first: {legacy}"
                    )
        destination = location(plan["target"], home)
        checked_parent(destination)
        owner_file = owner_path(state, destination)
        owner = read_json(owner_file, {"destination": str(destination), "skills": {}})
        if owner.get("destination") != str(destination) or not isinstance(
            owner.get("skills"), dict
        ):
            raise ValidationError(f"invalid ownership record: {owner_file}")
        # The manifest is authoritative: install replaces these after backing them up.
        for name in managed(plan, owner):
            tree(destination / name)
        items.append((plan, destination, owner_file, owner))
    return items


def managed(plan, owner):
    """Selected skills plus previously owned ones that installation removes."""
    return sorted(set(selected(plan)) | set(owner["skills"]))


def matches_disk(destination, owner):
    return all(
        tree(destination / name) == record.get("files")
        for name, record in owner["skills"].items()
    )


def validate_receipt(receipt, home, state):
    destinations = {target: location(target, home) for target in ("shared", "claude")}
    validate_installation_receipt(
        receipt,
        destinations=destinations,
        owner_files={target: owner_path(state, path) for target, path in destinations.items()},
        lock_files=lock_paths(home, state),
    )


def validate_restore_snapshot(receipt, backup):
    for path, snapshot in receipt["locks"].items():
        if "after_bytes" in snapshot:
            unchanged = Path(path).read_bytes().hex() == snapshot["after_bytes"]
        else:
            unchanged = lock_entries(Path(path), snapshot["after"]) == snapshot["after"]
        if not unchanged:
            raise ValidationError(
                f"selected CLI lock entries changed since backup: {path}"
            )
    for item in receipt["targets"]:
        destination = Path(item["destination"])
        checked_parent(destination)
        if "after_owner_bytes" in item:
            unchanged = (
                Path(item["owner_file"]).read_bytes().hex() == item["after_owner_bytes"]
            )
        else:
            unchanged = read_json(Path(item["owner_file"])) == item.get("after_owner")
        if not unchanged:
            raise ValidationError(
                f"ownership changed since backup: {item['owner_file']}"
            )
        for name, fingerprint in item["after"].items():
            if tree(destination / name) != fingerprint:
                raise ValidationError(
                    f"changed since backup; refusing restoration: {destination / name}"
                )
    for index, item in enumerate(receipt["targets"]):
        for name in item["after"]:
            old = backup / str(index) / name
            previous = item["before_owner"]["skills"].get(name)
            if tree(old) != (previous["files"] if previous else None):
                raise ValidationError(f"backup files changed: {old}")
            if (backup / "replaced" / str(index) / name).exists():
                raise ValidationError(
                    "restoration already attempted; preserve remaining backup"
                )


def restore(backup, home, state):
    receipt = read_json(backup / "operation.json")
    validate_receipt(receipt, home, state)
    validate_restore_snapshot(receipt, backup)
    for index, item in enumerate(receipt["targets"]):
        destination = Path(item["destination"])
        for name in item["after"]:
            current = destination / name
            old = backup / str(index) / name
            if current.exists():
                preserved = backup / "replaced" / str(index) / name
                preserved.parent.mkdir(parents=True, exist_ok=True)
                if preserved.exists():
                    raise ValidationError(f"restoration already attempted: {preserved}")
                shutil.move(str(current), preserved)
            if old.exists():
                shutil.copytree(old, current, copy_function=shutil.copy2)
        if item["before_owner"]["skills"]:
            write_json(Path(item["owner_file"]), item["before_owner"])
        else:
            Path(item["owner_file"]).unlink(missing_ok=True)
    for path, snapshot in receipt["locks"].items():
        restore_lock(Path(path), snapshot)
    receipt["status"] = "restored"
    write_json(backup / "operation.json", receipt)


def source_inputs(plans, home, state, overrides, replay=None):
    revs = revisions(plans, overrides, environment(home, state))
    local_hashes = {
        name: tree(Path(source["path"]) / relative, allow_hardlinks=True)
        for plan in plans
        for source in plan["sources"]
        if source["kind"] == "local"
        for name, relative in source["skills"].items()
    }
    if replay and (
        replay.get("revisions") != revs or replay.get("local_hashes") != local_hashes
    ):
        raise ValidationError(
            "replay source content differs from recorded installation"
        )
    return revs, local_hashes


def installation_state(plan, revs, local_hashes, home, state):
    names = selected(plan)
    return {
        "plan": plan,
        "revisions": {
            source["id"]: revs[source["id"]]
            for source in plan["sources"]
            if source["kind"] == "git"
        },
        "local_hashes": {
            name: local_hashes[name] for name in names if name in local_hashes
        },
        "locks": {
            str(path): lock_entries(path, names) for path in lock_paths(home, state)
        },
    }


def successful_owner(owner):
    path = owner.get("installation_receipt")
    if not isinstance(path, str):
        return False
    receipt = read_json(Path(path), {})
    return (
        isinstance(receipt, dict)
        and receipt.get("status") == "installed"
        and isinstance(receipt.get("targets"), list)
        and any(
            isinstance(item, dict)
            and item.get("destination") == owner["destination"]
            and item.get("after_owner") == owner
            for item in receipt.get("targets", [])
        )
    )


def prepare(plans, staging, overrides, replay=None, inputs=None):
    revs, local_hashes = (
        inputs
        if inputs is not None
        else source_inputs(plans, staging, staging / "state", overrides, replay)
    )
    sources = {}
    for plan in plans:
        for source in plan["sources"]:
            if source["id"] not in sources:
                sources[source["id"]] = {**source, "skills": dict(source["skills"])}
            else:
                sources[source["id"]]["skills"].update(source["skills"])
    notices = {
        identifier: license_bytes(source, revs[identifier])
        for identifier, source in sources.items()
        if source["kind"] == "git"
    }
    prepared = {}
    for plan in plans:
        stage_home = staging / plan["target"]
        for source in plan["sources"]:
            cli_add(
                plan,
                source,
                revs.get(source["id"]),
                stage_home,
                stage_home / "state",
                isolated=True,
            )
        add_licenses(plan, location(plan["target"], stage_home), notices)
        prepared[plan["target"]] = {
            name: tree(location(plan["target"], stage_home) / name)
            for name in selected(plan)
        }
    for source in sources.values():
        if source["kind"] == "local":
            for name, relative in source["skills"].items():
                if (
                    tree(Path(source["path"]) / relative, allow_hardlinks=True)
                    != local_hashes[name]
                ):
                    raise ValidationError(
                        f"local source changed during preparation: {name}"
                    )
    if replay and (
        replay.get("revisions") != revs or replay.get("local_hashes") != local_hashes
    ):
        raise ValidationError(
            "replay source content differs from recorded installation"
        )
    return revs, notices, prepared, local_hashes


def install(plans, home, state, overrides, replay=None):
    with (
        exclusive(state),
        tempfile.TemporaryDirectory(prefix="agent-skills-") as temporary,
    ):
        items = preflight(plans, home, state)
        inputs = source_inputs(plans, home, state, overrides, replay)
        revs, local_hashes = inputs
        items = preflight(plans, home, state)
        if all(
            set(owner["skills"]) == set(selected(plan))
            and owner.get("installation")
            == installation_state(plan, revs, local_hashes, home, state)
            and successful_owner(owner)
            and matches_disk(destination, owner)
            for plan, destination, _, owner in items
        ):
            return None
        revs, notices, prepared, local_hashes = prepare(
            plans, Path(temporary).resolve(), overrides, replay, inputs=inputs
        )
        # Recheck paths and ownership after network preparation, before touching files.
        items = preflight(plans, home, state)
        names = {name for plan, _, _, owner in items for name in managed(plan, owner)}
        locks = {
            str(path): {
                "before": lock_entries(path, names),
                "after": {},
                "existed": path.exists(),
                "before_bytes": path.read_bytes().hex()
                if path.exists()
                else b"{}".hex(),
            }
            for path in lock_paths(home, state)
        }
        backup_root = state / "agent-core/skills/backups"
        backup_root.mkdir(parents=True, exist_ok=True)
        backup = Path(tempfile.mkdtemp(prefix="install-", dir=backup_root))
        receipt = {
            "version": 1,
            "status": "prepared",
            "revisions": revs,
            "local_hashes": local_hashes,
            "targets": [],
            "locks": locks,
        }
        for index, (plan, destination, owner_file, owner) in enumerate(items):
            for name in managed(plan, owner):
                if (destination / name).exists():
                    shutil.copytree(destination / name, backup / str(index) / name)
            receipt["targets"].append(
                {
                    "target": plan["target"],
                    "destination": str(destination),
                    "owner_file": str(owner_file),
                    "before_owner": owner,
                    "after": {},
                }
            )
        write_json(backup / "operation.json", receipt)
        failure = None
        try:
            # A partial retry must never reuse a previous successful cache record.
            for _, _, owner_file, owner in items:
                write_json(
                    owner_file,
                    {
                        key: value
                        for key, value in owner.items()
                        if key not in ("installation", "installation_receipt")
                    },
                )
            for index, (plan, destination, owner_file, owner) in enumerate(items):
                for name in managed(plan, owner):
                    if (destination / name).exists():
                        shutil.rmtree(destination / name)
                for source in plan["sources"]:
                    cli_add(plan, source, revs.get(source["id"]), home, state)
                supplements = add_licenses(plan, destination, notices)
                records = {}
                for name, source in selected(plan).items():
                    actual = tree(destination / name)
                    if actual != prepared[plan["target"]][name]:
                        raise ValidationError(
                            f"staged/installed bytes differ: {destination / name}"
                        )
                    records[name] = {
                        "source": source["id"],
                        "revision": revs.get(source["id"]),
                        "local_hash": local_hashes.get(name),
                        "files": actual,
                        "license_supplement": supplements.get(name),
                    }
                write_json(
                    owner_file, {"destination": str(destination), "skills": records}
                )
            for plan, _, owner_file, _ in items:
                owner = read_json(owner_file)
                owner["installation"] = installation_state(
                    plan, revs, local_hashes, home, state
                )
                owner["installation_receipt"] = str(backup / "operation.json")
                write_json(owner_file, owner)
            receipt["status"] = "installed"
        except BaseException as error:
            failure = error
            receipt["status"] = "failed"
            receipt["error"] = str(error)
        try:
            receipt["snapshot_errors"] = []
            for item in receipt["targets"]:
                item["snapshot_errors"] = []
                for name in managed(
                    next(plan for plan in plans if plan["target"] == item["target"]),
                    item["before_owner"],
                ):
                    try:
                        item["after"][name] = tree(Path(item["destination"]) / name)
                    except (ValidationError, OSError) as error:
                        item["snapshot_errors"].append(str(error))
                receipt["snapshot_errors"].extend(item["snapshot_errors"])
                try:
                    item["after_owner"] = read_json(Path(item["owner_file"]))
                except ValidationError as error:
                    receipt["snapshot_errors"].append(str(error))
                    item["after_owner_bytes"] = (
                        Path(item["owner_file"]).read_bytes().hex()
                    )
            for path, snapshot in receipt["locks"].items():
                try:
                    snapshot["after"] = lock_entries(Path(path), names)
                except ValidationError as error:
                    receipt["snapshot_errors"].append(str(error))
                    snapshot["after_bytes"] = Path(path).read_bytes().hex()
        except (ValidationError, OSError) as error:
            receipt["snapshot_errors"].append(str(error))
            receipt["snapshot_incomplete"] = True
        if receipt["snapshot_errors"] and failure is None:
            failure = ValidationError("; ".join(receipt["snapshot_errors"]))
            receipt["status"] = "failed"
            receipt["error"] = str(failure)
        try:
            write_json(backup / "operation.json", receipt)
        except OSError as error:
            if failure is None:
                failure = error
            raise ValidationError(
                f"installation failed; could not save recovery receipt: {error}. Backup: {backup}\n{failure}"
            ) from failure
        if failure is not None:
            raise ValidationError(
                f"installation failed; files/locks may be partial. Restore backup: {backup}\n{failure}"
            ) from failure
        return backup


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["plan", "probe", "install", "restore"])
    parser.add_argument(
        "--target", action="append", choices=["shared", "claude"]
    )
    parser.add_argument(
        "--apply",
        action="store_true",
        help="Explicitly change installed files and CLI locks.",
    )
    parser.add_argument(
        "--revision", action="append", default=[], metavar="SOURCE=FULL_SHA"
    )
    parser.add_argument(
        "--replay",
        type=Path,
        help="Replay an operation.json with exact SHAs and unchanged local sources.",
    )
    parser.add_argument("--backup", type=Path)
    parser.add_argument(
        "--output",
        type=Path,
        help="New JSON receipt path for an isolated pre-switch probe.",
    )
    args = parser.parse_args(argv)
    try:
        home = Path.home().resolve()
        state = Path(
            os.environ.get("XDG_STATE_HOME", str(home / ".local/state"))
        ).absolute()
        if args.command == "restore":
            if not args.apply or not args.backup:
                raise ValidationError("restore requires --apply and --backup")
            with exclusive(state):
                restore(args.backup.resolve(), home, state)
            print("restored selected installation files, ownership, and lock entries")
            return 0
        if not args.target or len(args.target) != len(set(args.target)):
            raise ValidationError("specify each requested --target exactly once")
        plans = [installation_plan(target) for target in args.target]
        preview = [
            {**plan, "destination": str(location(plan["target"], home))}
            for plan in plans
        ]
        print(json.dumps(preview, indent=2, sort_keys=True), flush=True)
        if args.command == "plan":
            preflight(plans, home, state)
            return 0
        if args.command == "install" and not args.apply:
            raise ValidationError("install requires --apply; review plan first")
        overrides = {}
        for value in args.revision:
            key, separator, revision = value.partition("=")
            if not separator or key in overrides:
                raise ValidationError(f"invalid or duplicate revision: {value}")
            overrides[key] = revision
        replay = read_json(args.replay) if args.replay else None
        if replay:
            if overrides:
                raise ValidationError("--revision and --replay are mutually exclusive")
            overrides = replay["revisions"]
        if args.command == "probe":
            if not args.output or args.output.exists() or args.output.is_symlink():
                raise ValidationError("probe requires a new --output receipt file")
            with tempfile.TemporaryDirectory(prefix="agent-skills-probe-") as temporary:
                revs, _, _, local_hashes = prepare(
                    plans, Path(temporary).resolve(), overrides, replay
                )
            write_json(args.output, {"revisions": revs, "local_hashes": local_hashes})
            print(f"isolated installation verified; source receipt: {args.output}")
            return 0
        backup = install(plans, home, state, overrides, replay)
        if backup is None:
            print(
                "installation unchanged; verified sources, ownership, files, and CLI locks"
            )
        else:
            print(f"installation verified; backup and exact source receipt: {backup}")
        return 0
    except (ValidationError, OSError, KeyError, TypeError) as error:
        print(f"agent-skills: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
