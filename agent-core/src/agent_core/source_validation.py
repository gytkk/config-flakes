"""Validate source selections using explicitly supplied resource access."""

import re
from pathlib import PurePosixPath
from urllib.parse import urlsplit

from .validation import ValidationError, _expect_keys

TARGET_RUNTIMES = {
    "shared": ["codex", "pi", "omp"],
    "claude": ["claude"],
}


def _name(value: object) -> str:
    if not isinstance(value, str) or not re.fullmatch(
        r"[a-z0-9]+(?:-[a-z0-9]+)*", value
    ):
        raise ValidationError(f"invalid skill or source name: {value!r}")
    return value


def _path(value: object) -> str:
    if (
        not isinstance(value, str)
        or not value
        or any(
            not re.fullmatch(r"[A-Za-z0-9_.-]+", part) or part in (".", "..")
            for part in value.split("/")
        )
    ):
        raise ValidationError(f"unsafe skill source path: {value!r}")
    return value


def validate_sources(manifest: dict, *, source, materialize_skill) -> None:
    _source = source
    _materialize_skill = materialize_skill
    cli = _expect_keys(manifest["skill_cli"], {"version"}, "skill CLI")
    if cli["version"] != "1.7.2":
        raise ValidationError("skill CLI version must be pinned to 1.7.2")
    sources = manifest["skill_sources"]
    if not isinstance(sources, dict) or not sources:
        raise ValidationError("skill_sources must be a nonempty table")
    names: set[str] = set()
    for source_id, source in sources.items():
        _name(source_id)
        if not isinstance(source, dict):
            raise ValidationError(f"invalid skill source: {source_id}")
        kind = source.get("kind")
        if kind == "local":
            _expect_keys(source, {"kind", "path", "skills"}, f"source {source_id}")
            source_root = _source(_path(source["path"]), directory=True)
        elif kind == "git":
            _expect_keys(
                source,
                {"kind", "url", "update", "license", "skills"},
                f"source {source_id}",
            )
            url = source["url"]
            try:
                parsed = urlsplit(url) if isinstance(url, str) else None
            except ValueError as error:
                raise ValidationError(
                    f"invalid external source URL: {url!r}"
                ) from error
            if (
                parsed is None
                or parsed.scheme != "https"
                or not parsed.hostname
                or parsed.netloc != parsed.hostname
                or parsed.query
                or parsed.fragment
                or not parsed.path.endswith(".git")
            ):
                raise ValidationError(f"invalid external source URL: {url!r}")
            _path(parsed.path.removeprefix("/"))
            if source["update"] != "latest-default-branch":
                raise ValidationError(f"invalid external update policy: {source_id}")
            _path(source["license"])
        else:
            raise ValidationError(f"invalid source kind: {kind!r}")
        if not isinstance(source["skills"], dict) or not source["skills"]:
            raise ValidationError(
                f"source {source_id} requires a nonempty skills table"
            )
        paths: set[str] = set()
        for name, relative in source["skills"].items():
            _name(name)
            _path(relative)
            if name in names:
                raise ValidationError(f"duplicate skill name across sources: {name}")
            if relative in paths:
                raise ValidationError(
                    f"duplicate skill path in source {source_id}: {relative}"
                )
            names.add(name)
            paths.add(relative)
            if kind == "local":
                local = _source(f"{source['path']}/{relative}", directory=True)
                if not local.resolve().is_relative_to(source_root.resolve()):
                    raise ValidationError(f"unsafe local skill path: {relative}")
                _materialize_skill({}, PurePosixPath("skills"), name, local, b"")
    targets = _expect_keys(
        manifest["skill_targets"], set(TARGET_RUNTIMES), "skill targets"
    )
    for target, runtimes in TARGET_RUNTIMES.items():
        config = _expect_keys(
            targets[target], {"runtimes", "skills"}, f"target {target}"
        )
        if config["runtimes"] != runtimes:
            raise ValidationError(f"invalid runtimes for target: {target}")
        selected = config["skills"]
        if not isinstance(selected, list) or not selected:
            raise ValidationError(f"target {target} requires a nonempty skills array")
        seen: set[str] = set()
        for name in selected:
            _name(name)
            if name in seen:
                raise ValidationError(f"duplicate skill in target {target}: {name}")
            if name not in names:
                raise ValidationError(f"unknown skill in target {target}: {name}")
            seen.add(name)


