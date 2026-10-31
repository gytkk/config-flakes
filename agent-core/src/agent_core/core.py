from __future__ import annotations

import hashlib
import os
import re
from collections.abc import Mapping
from pathlib import Path, PurePosixPath

import tomllib

from .validation import ValidationError, _expect_keys
from .source_validation import validate_sources

RUNTIMES = ("codex", "claude", "pi")
_SOURCE_ROOT = Path(__file__).resolve().parents[2]
_PACKAGED_ROOT = Path(__file__).resolve().parent / "resources"
_FRONTMATTER = re.compile(r"\A---\r?\n(.*?)\r?\n---\r?\n", re.DOTALL)


def resource_root() -> Path:
    if (_SOURCE_ROOT / "manifest.toml").is_file():
        return _SOURCE_ROOT
    if (_PACKAGED_ROOT / "manifest.toml").is_file():
        return _PACKAGED_ROOT
    raise ValidationError("agent-core resources are missing")


def _safe_relative(value: str) -> PurePosixPath:
    path = PurePosixPath(value)
    if (
        path.is_absolute()
        or not path.parts
        or any(part in ("", ".", "..") for part in path.parts)
    ):
        raise ValidationError(f"unsafe source or output path: {value!r}")
    return path


def _load_manifest() -> dict:
    path = resource_root() / "manifest.toml"
    try:
        with path.open("rb") as handle:
            manifest = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ValidationError(f"invalid manifest: {error}") from error

    _expect_keys(
        manifest,
        {
            "metadata",
            "documents",
            "runtimes",
            "skill_cli",
            "skill_sources",
            "skill_targets",
        },
        "manifest",
    )
    _expect_keys(
        manifest["metadata"],
        {"marker", "version"},
        "manifest metadata",
    )
    if not isinstance(manifest["documents"], list):
        raise ValidationError("documents must be an array")
    for index, document_value in enumerate(manifest["documents"]):
        document = _expect_keys(
            document_value,
            {"runtime", "output", "sources"},
            f"document {index}",
        )
        if document["runtime"] not in RUNTIMES:
            raise ValidationError(f"document {index} names an unsupported runtime")
    if not isinstance(manifest["runtimes"], dict) or set(manifest["runtimes"]) != set(
        RUNTIMES
    ):
        raise ValidationError("manifest must configure exactly the supported runtimes")
    for runtime, runtime_config in manifest["runtimes"].items():
        _expect_keys(runtime_config, {"skill_output", "skills"}, f"runtime {runtime}")
    validate_sources(manifest, source=_source, materialize_skill=_materialize_skill)
    return manifest


def _source(path: str, *, directory: bool = False) -> Path:
    relative = _safe_relative(path)
    root = resource_root()
    candidate = root.joinpath(*relative.parts)
    try:
        resolved = candidate.resolve(strict=True)
    except OSError as error:
        raise ValidationError(f"missing source path: {path}") from error
    if not resolved.is_relative_to(root.resolve()) or candidate.is_symlink():
        raise ValidationError(f"unsafe source path: {path}")
    if (directory and not candidate.is_dir()) or (
        not directory and not candidate.is_file()
    ):
        raise ValidationError(f"invalid source type: {path}")
    return candidate


def _skill_body(path: Path, skill_name: str, marker: bytes) -> bytes:
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        raise ValidationError(
            f"invalid UTF-8 skill document: {path}: {error}"
        ) from error
    match = _FRONTMATTER.match(text)
    if not match:
        raise ValidationError(f"missing or invalid SKILL.md frontmatter: {path}")
    frontmatter = match.group(1)
    name = re.search(r"^name:\s*([^\r\n]+)\s*$", frontmatter, re.MULTILINE)
    description = re.search(
        r"^description:\s*(?:[^\r\n]+|[>|][+-]?)", frontmatter, re.MULTILINE
    )
    if not name or not description:
        raise ValidationError(f"missing name or description in SKILL.md: {path}")
    if name.group(1).strip(" '\"") != skill_name:
        raise ValidationError(f"skill directory-name/name mismatch: {skill_name}")
    return (
        text[: match.end()] + "\n" + marker.decode("utf-8") + "\n" + text[match.end() :]
    ).encode()


def _put(
    result: dict[PurePosixPath, bytes], path: PurePosixPath, content: bytes
) -> None:
    if path in result:
        raise ValidationError(f"duplicate output path: {path}")
    result[path] = content


def _skill_name(value: object) -> str:
    if not isinstance(value, str) or "/" in value or value in ("", ".", ".."):
        raise ValidationError(f"invalid skill name: {value!r}")
    return value


def _materialize_skill(
    result: dict[PurePosixPath, bytes],
    skill_root: PurePosixPath,
    skill_name: str,
    source_root: Path,
    generated: bytes,
) -> None:
    skill_document = source_root / "SKILL.md"
    if not skill_document.is_file() or skill_document.is_symlink():
        raise ValidationError(f"missing SKILL.md: {skill_name}")
    for source_file in sorted(source_root.rglob("*")):
        if source_file.is_symlink():
            raise ValidationError(f"unsafe skill source path: {source_file}")
        if source_file.is_dir():
            continue
        if not source_file.is_file():
            raise ValidationError(f"invalid skill source path: {source_file}")
        relative = PurePosixPath(source_file.relative_to(source_root).as_posix())
        output = skill_root / skill_name / relative
        content = (
            _skill_body(source_file, skill_name, generated)
            if relative == PurePosixPath("SKILL.md")
            else source_file.read_bytes()
        )
        _put(result, output, content)


def materialize(runtime: str) -> Mapping[PurePosixPath, bytes]:
    """Return deterministic generated files for a supported runtime."""
    manifest = _load_manifest()
    runtimes = manifest["runtimes"]
    if runtime not in RUNTIMES or runtime not in runtimes:
        raise ValidationError(f"unknown runtime: {runtime}")
    metadata = manifest["metadata"]
    marker_name = metadata.get("marker")
    version = metadata.get("version")
    if not isinstance(marker_name, str) or not isinstance(version, str):
        raise ValidationError("manifest metadata requires string marker and version")
    marker_path = _safe_relative(marker_name)
    generated = f"<!-- Generated by agent-core v{version}; do not edit. -->\n".encode()
    result: dict[PurePosixPath, bytes] = {}
    _put(result, marker_path, f"agent-core v{version}\n".encode())

    documents = manifest.get("documents")
    if not isinstance(documents, list):
        raise ValidationError("manifest documents must be an array")
    for document in documents:
        if not isinstance(document, dict):
            raise ValidationError("invalid document entry")
        document_runtime = document.get("runtime")
        if document_runtime not in RUNTIMES:
            raise ValidationError(f"unknown document runtime: {document_runtime!r}")
        if document_runtime != runtime:
            continue
        output = document.get("output")
        sources = document.get("sources")
        if (
            not isinstance(output, str)
            or not isinstance(sources, list)
            or not all(isinstance(item, str) for item in sources)
        ):
            raise ValidationError("invalid document entry")
        body = b"\n\n".join(_source(item).read_bytes() for item in sources)
        _put(result, _safe_relative(output), generated + b"\n" + body)

    config = runtimes[runtime]
    if (
        not isinstance(config, dict)
        or not isinstance(config.get("skills"), list)
        or not isinstance(config.get("skill_output"), str)
    ):
        raise ValidationError(f"invalid runtime configuration: {runtime}")
    skill_root = _safe_relative(config["skill_output"])
    skill_sources: dict[str, Path] = {}
    for configured_name in config["skills"]:
        skill_name = _skill_name(configured_name)
        source_root = _source(f"skills/{skill_name}", directory=True)
        if skill_name in skill_sources:
            raise ValidationError(f"duplicate skill name: {skill_name}")
        skill_sources[skill_name] = source_root
    for skill_name, source_root in sorted(skill_sources.items()):
        _materialize_skill(result, skill_root, skill_name, source_root, generated)
    return dict(sorted(result.items(), key=lambda item: item[0].as_posix()))


def output_hash(files: Mapping[PurePosixPath, bytes]) -> str:
    digest = hashlib.sha256()
    for path, content in sorted(files.items(), key=lambda item: item[0].as_posix()):
        digest.update(path.as_posix().encode() + b"\0" + content + b"\0")
    return digest.hexdigest()


def render(runtime: str, output: Path) -> None:
    if output.is_symlink() or (
        output.exists() and (not output.is_dir() or any(output.iterdir()))
    ):
        raise ValidationError(f"output directory must be missing or empty: {output}")
    files = materialize(runtime)
    output.mkdir(parents=True, exist_ok=True)
    for relative, content in files.items():
        destination = output.joinpath(*relative.parts)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(content)


def compare_trees(expected: Path, actual: Path) -> list[str]:
    def entries(root: Path) -> dict[PurePosixPath, tuple[str, bytes | str | None]]:
        if not root.is_dir():
            raise ValidationError(f"tree is not a directory: {root}")
        found: dict[PurePosixPath, tuple[str, bytes | str | None]] = {}
        for base, directories, names in os.walk(root, followlinks=False):
            base_path = Path(base)
            for name in directories + names:
                path = base_path / name
                relative = PurePosixPath(path.relative_to(root).as_posix())
                if path.is_symlink():
                    found[relative] = ("symlink", os.readlink(path))
                elif path.is_dir():
                    found[relative] = ("directory", None)
                elif path.is_file():
                    found[relative] = ("file", path.read_bytes())
                else:
                    found[relative] = ("other", None)
        return found

    expected_entries, actual_entries = entries(expected), entries(actual)
    differences: list[str] = []
    for path in sorted(expected_entries.keys() - actual_entries.keys()):
        differences.append(f"missing: {path}")
    for path in sorted(actual_entries.keys() - expected_entries.keys()):
        differences.append(f"unexpected: {path}")
    for path in sorted(expected_entries.keys() & actual_entries.keys()):
        expected_kind, expected_value = expected_entries[path]
        actual_kind, actual_value = actual_entries[path]
        if expected_kind != actual_kind:
            differences.append(
                f"type differs: {path} ({expected_kind} != {actual_kind})"
            )
        elif expected_value != actual_value:
            label = (
                "symlink target differs"
                if expected_kind == "symlink"
                else "bytes differ"
            )
            differences.append(f"{label}: {path}")
    return differences
