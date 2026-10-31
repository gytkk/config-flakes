from __future__ import annotations

import hashlib
import tomllib
from pathlib import PurePosixPath

from .core import (
    ValidationError,
    _expect_keys,
    _load_manifest,
    _materialize_skill,
    _source,
    materialize,
    resource_root,
)

from .source_validation import TARGET_RUNTIMES, validate_sources as _validate_sources


def validate_sources(manifest: dict) -> None:
    _validate_sources(manifest, source=_source, materialize_skill=_materialize_skill)


def installation_plan(target: str) -> dict:
    """Return a read-only CLI installation plan without consulting host state."""
    if target not in TARGET_RUNTIMES:
        raise ValidationError(f"unknown skill target: {target}")
    manifest = _load_manifest()
    config = manifest["skill_targets"][target]
    selected = set(config["skills"])
    sources = []
    for source_id, source in manifest["skill_sources"].items():
        skills = {
            name: path
            for name, path in sorted(source["skills"].items())
            if name in selected
        }
        if not skills:
            continue
        entry = {"id": source_id, **source, "skills": skills}
        if source["kind"] == "local":
            entry["path"] = str(_source(source["path"], directory=True).resolve())
        sources.append(entry)
    return {
        "cli_version": manifest["skill_cli"]["version"],
        "target": target,
        "runtimes": config["runtimes"],
        "sources": sources,
    }


def check_document_hashes() -> list[str]:
    """Compare instruction bytes independently of the legacy skill tree hashes."""
    manifest = _load_manifest()
    path = resource_root() / "schema" / "document-hashes.toml"
    try:
        with path.open("rb") as handle:
            fixture = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ValidationError(f"invalid document hash fixture: {error}") from error
    hashes = _expect_keys(fixture, {"hashes"}, "document hash fixture")["hashes"]
    expected_paths: dict[str, set[str]] = {}
    for document in manifest["documents"]:
        expected_paths.setdefault(document["runtime"], set()).add(document["output"])
    _expect_keys(hashes, set(expected_paths), "document hash runtimes")
    errors = []
    for runtime, documents in expected_paths.items():
        expected = _expect_keys(
            hashes[runtime], documents, f"document hashes for {runtime}"
        )
        files = materialize(runtime)
        for document in sorted(documents):
            digest = hashlib.sha256(files[PurePosixPath(document)]).hexdigest()
            if expected[document] != digest:
                errors.append(
                    f"{runtime}/{document}: instruction document hash differs"
                )
    return errors
