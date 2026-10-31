"""Version-one installation receipt validation, independent of filesystem mutation."""
from __future__ import annotations

import json
import re

from .core import ValidationError

SHA = re.compile(r"[0-9a-f]{40}\Z")


def validate_receipt(receipt, *, destinations, owner_files, lock_files):
    if (
        not isinstance(receipt, dict)
        or receipt.get("version") != 1
        or receipt.get("status") not in ("installed", "failed")
    ):
        raise ValidationError("backup is invalid, incomplete, or already restored")
    if receipt.get("snapshot_incomplete"):
        raise ValidationError(
            "incomplete metadata snapshot requires manual preservation before restoration"
        )
    if not isinstance(receipt.get("revisions"), dict) or any(
        not isinstance(value, str) or not SHA.fullmatch(value)
        for value in receipt["revisions"].values()
    ):
        raise ValidationError("backup contains an invalid full source SHA")
    targets = receipt.get("targets")
    if not isinstance(targets, list) or not targets:
        raise ValidationError("backup has no installation targets")
    seen = set()
    names = set()
    for item in targets:
        if (
            not isinstance(item, dict)
            or item.get("target") not in ("shared", "claude")
            or item["target"] in seen
        ):
            raise ValidationError("backup has invalid or duplicate targets")
        seen.add(item["target"])
        destination = destinations[item["target"]]
        if item.get("destination") != str(destination) or item.get("owner_file") != str(
            owner_files[item["target"]]
        ):
            raise ValidationError(
                "backup paths do not match the current home and state directory"
            )
        if item.get("snapshot_errors"):
            raise ValidationError(
                f"unsafe partial output requires manual preservation before restoration: {item['snapshot_errors']}"
            )
        if not isinstance(item.get("after"), dict) or not item["after"]:
            raise ValidationError("backup has no completed file snapshot")
        for name in item["after"]:
            if not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", name):
                raise ValidationError(f"unsafe skill name in backup: {name}")
            names.add(name)
        before = item.get("before_owner")
        if "after_owner_bytes" in item:
            validate_bytes(item["after_owner_bytes"])
        if (
            not isinstance(before, dict)
            or before.get("destination") != str(destination)
            or not isinstance(before.get("skills"), dict)
            or set(before["skills"]) - item["after"].keys()
        ):
            raise ValidationError("invalid previous ownership in backup")
        for record in before["skills"].values():
            revision = record.get("revision")
            if revision is not None and (
                not isinstance(revision, str) or not SHA.fullmatch(revision)
            ):
                raise ValidationError("invalid previous ownership SHA in backup")
    locks = receipt.get("locks")
    expected_locks = {str(path) for path in lock_files}
    if not isinstance(locks, dict) or set(locks) != expected_locks:
        raise ValidationError("backup CLI lock paths do not match current installation")
    for snapshot in locks.values():
        if (
            not isinstance(snapshot, dict)
            or not {"before", "after", "existed"} <= set(snapshot)
            or set(snapshot)
            - {"before", "after", "existed", "before_bytes", "after_bytes"}
            or not isinstance(snapshot["existed"], bool)
            or any(
                not isinstance(snapshot[key], dict) or set(snapshot[key]) != names
                for key in (
                    ("before",) if "after_bytes" in snapshot else ("before", "after")
                )
            )
        ):
            raise ValidationError("invalid selected CLI lock snapshot")
        if "after_bytes" in snapshot:
            validate_bytes(snapshot["after_bytes"])
            validate_bytes(snapshot.get("before_bytes"))
            try:
                before = json.loads(bytes.fromhex(snapshot["before_bytes"]))
            except (ValueError, UnicodeError) as error:
                raise ValidationError("invalid previous CLI lock bytes") from error
            if (
                not isinstance(before, dict)
                or not isinstance(before.get("skills", {}), dict)
                or {name: before.get("skills", {}).get(name) for name in names}
                != snapshot["before"]
            ):
                raise ValidationError("invalid previous CLI lock bytes")


def validate_bytes(value):
    if not isinstance(value, str) or not re.fullmatch(r"(?:[0-9a-f]{2})*", value):
        raise ValidationError("invalid metadata byte snapshot")


