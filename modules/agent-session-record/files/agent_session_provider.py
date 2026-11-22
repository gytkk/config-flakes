from __future__ import annotations

import json
import re
import time
from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Protocol


@dataclass(frozen=True)
class HookContract:
    mode: str
    returns_continue: bool


@dataclass(frozen=True)
class ReplaySession:
    session_id: str
    transcript: Path
    cwd: str
    reason: str = ""


@dataclass(frozen=True)
class CaptureContext:
    home: Path
    config: Mapping[str, str]


@dataclass(frozen=True)
class CaptureSource:
    session_id: str
    transcript: Path
    cwd: str
    hook_event_name: str
    source: str
    provider_identity: str
    parent_identity: str | None
    relationship_status: str
    model: str
    agent_role: str
    started_at: str
    termination_status: str
    manifest_fields: Mapping[str, Any] = field(default_factory=dict)
    delete_after_capture: bool = False


class ProviderAdapter(Protocol):
    name: str
    hook_events: tuple[str, ...]

    def hook_contract(self, event: str) -> HookContract: ...

    def captures_from_event(
        self, payload: Mapping[str, object], context: CaptureContext
    ) -> list[CaptureSource]: ...

    def discover_captures(self, context: CaptureContext) -> list[CaptureSource]: ...

    def sessions_dir(self, home: Path, config: dict[str, str]) -> Path: ...

    def discover_replay_sessions(
        self, home: Path, config: dict[str, str]
    ) -> Sequence[ReplaySession | None]: ...

    def replay_payload(self, session: ReplaySession) -> dict[str, Any]: ...


def jsonl_objects(path: Path) -> list[dict[str, Any]]:
    objects: list[dict[str, Any]] = []
    try:
        with path.open(encoding="utf-8") as handle:
            for line in handle:
                try:
                    value = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if isinstance(value, dict):
                    objects.append(value)
    except (OSError, UnicodeError):
        pass
    return objects


def first_record(path: Path) -> dict[str, Any]:
    try:
        with path.open(encoding="utf-8") as handle:
            value = json.loads(handle.readline())
    except (OSError, json.JSONDecodeError, UnicodeError):
        return {}
    return value if isinstance(value, dict) else {}


def nested(value: dict[str, Any], *keys: str) -> Any:
    current: Any = value
    for key in keys:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    return current


def first_jsonl_value(path: Path, reader: Any) -> str:
    for item in jsonl_objects(path):
        value = reader(item)
        if isinstance(value, str) and value:
            return value
    return ""


def safe_id(value: object) -> bool:
    return (
        isinstance(value, str)
        and re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,127}", value) is not None
    )


def session_parts(path: Path, root: Path) -> tuple[str, ...]:
    if not path.is_absolute() or not path.is_relative_to(root):
        raise ValueError("transcript outside session directory")
    relative = path.relative_to(root)
    if ".." in relative.parts or path.resolve() != root.resolve() / relative:
        raise ValueError("unsafe transcript path")
    return relative.parts


class EphemeralSession(Exception):
    """The runtime explicitly reports that it has no transcript."""


class ClaudeProvider:
    name = "claude"
    hook_events = ("session-end",)

    def hook_contract(self, event: str) -> HookContract:
        if event == "session-end":
            return HookContract(mode="payload", returns_continue=False)
        raise ValueError(f"unsupported Claude hook event: {event}")

    def sessions_dir(self, home: Path, config: dict[str, str]) -> Path:
        return Path(config.get('CLAUDE_CONFIG_DIR', home / '.claude')) / 'projects'

    def captures_from_event(
        self, payload: Mapping[str, object], context: CaptureContext
    ) -> list[CaptureSource]:
        session_id = payload.get("session_id")
        transcript_raw = payload.get("transcript_path")
        if not isinstance(session_id, str) or not isinstance(transcript_raw, str):
            raise TypeError("Claude payload missing session_id or transcript_path")
        if transcript_raw == "":
            raise EphemeralSession
        transcript = Path(transcript_raw)
        root = self.sessions_dir(context.home, dict(context.config))
        parts = session_parts(transcript, root)
        child = (
            len(parts) == 4
            and parts[2] == "subagents"
            and transcript.name.startswith("agent-")
        )
        if not safe_id(session_id) or not (
            (len(parts) == 2 and transcript.name == f"{session_id}.jsonl")
            or (
                child
                and transcript.suffix == ".jsonl"
                and session_id in (parts[1], transcript.stem[6:])
            )
        ):
            raise ValueError("Claude transcript does not match session")
        if payload.get("_verified_no_persistence") is True:
            raise EphemeralSession
        for delay in (0, 0.1, 0.3, 0.6):
            if delay:
                time.sleep(delay)
            if transcript.is_file():
                break
        else:
            raise ValueError("Claude transcript still missing after retry")
        session_parts(transcript, root)
        if child:
            parent_id = first_jsonl_value(
                transcript, lambda item: item.get("sessionId")
            )
            return [self._subagent_capture(transcript, parent_id)]

        return self.direct_captures(payload, transcript, session_id, root)

    def direct_captures(
        self,
        payload: Mapping[str, object],
        transcript: Path,
        session_id: str,
        root: Path | None = None,
    ) -> list[CaptureSource]:
        recorded_id = first_jsonl_value(transcript, lambda item: item.get("sessionId"))
        if not safe_id(session_id) or (recorded_id and recorded_id != session_id):
            raise ValueError('Claude transcript session id mismatch')
        model = first_jsonl_value(
            transcript,
            lambda item: (
                nested(item, "message", "model")
                if item.get("type") == "assistant"
                else None
            ),
        )
        started_at = first_jsonl_value(transcript, lambda item: item.get("timestamp"))
        raw_reason = payload.get("reason")
        reason = raw_reason if isinstance(raw_reason, str) else ""
        raw_cwd = payload.get("cwd")
        cwd = raw_cwd if isinstance(raw_cwd, str) else ""
        captures = [
            CaptureSource(
                session_id=session_id,
                transcript=transcript,
                cwd=cwd,
                hook_event_name=str(payload.get("hook_event_name") or "SessionEnd"),
                source="session-end",
                provider_identity=session_id,
                parent_identity=None,
                relationship_status="direct",
                model=model,
                agent_role="direct",
                started_at=started_at,
                termination_status=reason or "captured",
                manifest_fields={"end_reason": reason},
            )
        ]
        subagents = transcript.with_suffix("") / "subagents"
        if root is not None and subagents.is_dir():
            session_parts(subagents, root)
            for child in sorted(subagents.glob("agent-*.jsonl")):
                session_parts(child, root)
                if not child.is_file():
                    raise ValueError('invalid Claude subagent file')
                captures.append(self._subagent_capture(child, session_id))
        return captures

    def discover_captures(self, context: CaptureContext) -> list[CaptureSource]:
        captures: list[CaptureSource] = []
        config = dict(context.config)
        for session in self.discover_replay_sessions(context.home, config):
            captures.extend(
                self.captures_from_event(self.replay_payload(session), context)
            )
        return captures

    def _subagent_capture(
        self, transcript: Path, parent_session_id: str
    ) -> CaptureSource:
        agent_id = first_jsonl_value(transcript, lambda item: item.get("agentId"))
        if not agent_id and transcript.stem.startswith("agent-"):
            agent_id = transcript.stem.removeprefix("agent-")
        if not safe_id(agent_id) or transcript.stem != f"agent-{agent_id}":
            raise ValueError("Claude subagent transcript id mismatch")
        recorded_parent = first_jsonl_value(
            transcript, lambda item: item.get("sessionId")
        )
        if recorded_parent and recorded_parent != transcript.parent.parent.name:
            raise ValueError('Claude subagent transcript parent mismatch')
        relationship = "identified" if parent_session_id else "unknown"
        model = first_jsonl_value(
            transcript,
            lambda item: (
                nested(item, "message", "model")
                if item.get("type") == "assistant"
                else None
            ),
        )
        identity = f"{parent_session_id or 'unknown'}:{agent_id}"
        return CaptureSource(
            session_id=agent_id,
            transcript=transcript,
            cwd=first_jsonl_value(transcript, lambda item: item.get("cwd")),
            hook_event_name="SubagentSweep",
            source="claude-subagent",
            provider_identity=identity,
            parent_identity=parent_session_id or None,
            relationship_status=relationship,
            model=model,
            agent_role="subagent",
            started_at=first_jsonl_value(
                transcript, lambda item: item.get("timestamp")
            ),
            termination_status="captured",
            manifest_fields={"end_reason": ""},
        )

    def discover_replay_sessions(
        self, home: Path, config: dict[str, str]
    ) -> list[ReplaySession]:
        projects_dir = self.sessions_dir(home, config)
        transcripts = projects_dir.glob("*/*/subagents/agent-*.jsonl")
        direct = set(projects_dir.glob('*/*.jsonl'))
        orphans: set[Path] = set()
        for path in transcripts:
            if path.parent.name != "subagents" or not path.name.startswith("agent-"):
                continue
            parent_transcript = path.parent.parent.with_suffix(".jsonl")
            if not parent_transcript.is_file():
                orphans.add(path)

        sessions: list[ReplaySession] = []
        for transcript in sorted(direct | orphans):
            session_parts(transcript, projects_dir)
            if not transcript.is_file():
                continue
            objects = jsonl_objects(transcript)
            if transcript.parent.name == "subagents":
                session_id = next(
                    (
                        item["agentId"]
                        for item in objects
                        if isinstance(item.get("agentId"), str) and item["agentId"]
                    ),
                    "",
                )
                if not session_id and transcript.stem.startswith("agent-"):
                    session_id = transcript.stem.removeprefix("agent-")
            else:
                session_id = transcript.stem
            cwd = next(
                (
                    item["cwd"]
                    for item in objects
                    if isinstance(item.get("cwd"), str) and item["cwd"]
                ),
                "",
            )
            reason = next(
                (
                    summary.get("stop_reason") or summary.get("reason")
                    for item in objects
                    if item.get("type") == "summary"
                    and isinstance((summary := item.get("summary")), dict)
                    and (summary.get("stop_reason") or summary.get("reason"))
                ),
                "",
            )
            sessions.append(
                ReplaySession(
                    session_id=session_id,
                    transcript=transcript,
                    cwd=cwd,
                    reason=str(reason),
                )
            )
        return sessions

    def replay_payload(self, session: ReplaySession) -> dict[str, Any]:
        return {
            "session_id": session.session_id,
            "transcript_path": str(session.transcript),
            "cwd": session.cwd,
            "hook_event_name": "ManualReplay",
            "reason": session.reason,
        }


class CodexProvider:
    name = "codex"
    hook_events = ("session-start", "stop")

    def hook_contract(self, event: str) -> HookContract:
        if event == "stop":
            return HookContract(mode="payload", returns_continue=True)
        if event == "session-start":
            return HookContract(mode="session-start-sweep", returns_continue=True)
        raise ValueError(f"unsupported Codex hook event: {event}")

    def sessions_dir(self, home: Path, config: dict[str, str]) -> Path:
        return Path(
            config.get(
                "AGENT_SESSION_RECORD_CODEX_SESSIONS_DIR", home / ".codex/sessions"
            )
        )

    def captures_from_event(
        self, payload: Mapping[str, object], context: CaptureContext
    ) -> list[CaptureSource]:
        session_id = payload.get("session_id")
        if not isinstance(session_id, str) or not session_id:
            raise ValueError("Codex payload missing session_id")
        transcript_raw = payload.get("transcript_path")
        transcript = Path(transcript_raw) if isinstance(transcript_raw, str) else None
        if transcript is None or not transcript.is_file():
            candidates = list(
                self.sessions_dir(context.home, dict(context.config)).rglob(
                    f"rollout-*-{session_id}.jsonl"
                )
            )
            transcript = (
                max(candidates, key=lambda path: path.stat().st_mtime_ns)
                if candidates
                else None
            )
        if transcript is None or not transcript.is_file():
            raise ValueError(f"Codex transcript missing for session {session_id}")

        first = first_record(transcript)
        transcript_session_id = nested(first, "payload", "id")
        if isinstance(transcript_session_id, str) and transcript_session_id:
            session_id = transcript_session_id
        source = nested(first, "payload", "source")
        parent_id = nested(
            first, "payload", "source", "subagent", "thread_spawn", "parent_thread_id"
        ) or nested(first, "payload", "parent_thread_id")
        is_subagent = (
            isinstance(source, dict) and source.get("subagent") is not None
        ) or nested(first, "payload", "parent_thread_id") is not None
        relationship = "direct"
        role = "direct"
        parent_identity = None
        if is_subagent:
            role = "subagent"
            relationship = "unknown"
            if isinstance(parent_id, str) and parent_id:
                parent_identity = parent_id
                relationship = "identified"

        model = first_jsonl_value(
            transcript,
            lambda item: (
                nested(item, "payload", "model")
                if item.get("type") == "turn_context"
                else None
            ),
        )
        started_at = nested(first, "payload", "timestamp")
        raw_cwd = payload.get("cwd")
        cwd = raw_cwd if isinstance(raw_cwd, str) else ""
        return [
            CaptureSource(
                session_id=session_id,
                transcript=transcript,
                cwd=cwd,
                hook_event_name=str(payload.get("hook_event_name") or "Stop"),
                source="stop",
                provider_identity=session_id,
                parent_identity=parent_identity,
                relationship_status=relationship,
                model=model,
                agent_role=role,
                started_at=started_at if isinstance(started_at, str) else "",
                termination_status="snapshot",
                manifest_fields={
                    "turn_id": str(payload.get("turn_id") or "") or None,
                    "stop_hook_active": payload.get("stop_hook_active") is True,
                    "last_assistant_message": str(
                        payload.get("last_assistant_message") or ""
                    )
                    or None,
                },
            )
        ]

    def discover_captures(self, context: CaptureContext) -> list[CaptureSource]:
        captures: list[CaptureSource] = []
        for transcript in sorted(
            self.sessions_dir(context.home, dict(context.config)).rglob(
                "rollout-*.jsonl"
            )
        ):
            if transcript.stat().st_size == 0:
                continue
            first = first_record(transcript)
            session_id = nested(first, "payload", "id")
            if not isinstance(session_id, str) or not session_id:
                continue
            cwd = nested(first, "payload", "cwd")
            captures.extend(
                self.captures_from_event(
                    {
                        "session_id": session_id,
                        "transcript_path": str(transcript),
                        "cwd": cwd if isinstance(cwd, str) else "",
                        "hook_event_name": "SessionStart",
                        "turn_id": "",
                        "stop_hook_active": False,
                        "last_assistant_message": "",
                    },
                    context,
                )
            )
        return captures

    def discover_replay_sessions(
        self, home: Path, config: dict[str, str]
    ) -> list[ReplaySession]:
        sessions: list[ReplaySession] = []
        for transcript in sorted(
            self.sessions_dir(home, config).rglob("rollout-*.jsonl")
        ):
            first = first_record(transcript)
            session_id = nested(first, "payload", "id")
            cwd = nested(first, "payload", "cwd")
            sessions.append(
                ReplaySession(
                    session_id=session_id if isinstance(session_id, str) else "",
                    transcript=transcript,
                    cwd=cwd if isinstance(cwd, str) else "",
                )
            )
        return sessions

    def replay_payload(self, session: ReplaySession) -> dict[str, Any]:
        return {
            "session_id": session.session_id,
            "transcript_path": str(session.transcript),
            "cwd": session.cwd,
            "hook_event_name": "ManualReplay",
            "turn_id": "",
            "stop_hook_active": False,
            "last_assistant_message": "",
        }


class PiProvider:
    name = "pi"
    hook_events = ("session-shutdown",)

    def hook_contract(self, event: str) -> HookContract:
        if event not in self.hook_events:
            raise ValueError("unsupported Pi hook event")
        return HookContract(mode="payload", returns_continue=False)

    def sessions_dir(self, home: Path, config: dict[str, str]) -> Path:
        return home / ".pi/agent/sessions"

    def captures_from_event(
        self, payload: Mapping[str, object], context: CaptureContext
    ) -> list[CaptureSource]:
        path = payload.get("transcript_path")
        session_id = payload.get("session_id")
        if path in (None, "") or payload.get("private", False) is not False:
            raise EphemeralSession
        if (
            not isinstance(path, str)
            or not isinstance(session_id, str)
            or not safe_id(session_id)
        ):
            raise ValueError("invalid Pi session reference")
        transcript = Path(path)
        parts = session_parts(
            transcript, self.sessions_dir(context.home, dict(context.config))
        )
        if (
            len(parts) != 2
            or not (parts[0].startswith("--") and parts[0].endswith("--"))
            or transcript.name.partition("_")[2] != f"{session_id}.jsonl"
            or not transcript.is_file()
        ):
            raise ValueError("invalid Pi session file")
        self.check_header(first_record(transcript), session_id)
        return [
            CaptureSource(
                session_id=session_id,
                transcript=transcript,
                cwd="",
                hook_event_name="ManualReplay" if payload.get("hook_event_name") == "ManualReplay" else "session_shutdown",
                source="session-shutdown",
                provider_identity=session_id,
                parent_identity=None,
                relationship_status="direct",
                model="",
                agent_role="direct",
                started_at="",
                termination_status="snapshot",
            )
        ]

    @staticmethod
    def check_header(value: object, session_id: str) -> None:
        if (
            not isinstance(value, dict)
            or value.get("type") != "session"
            or value.get("version") != 3
            or value.get("id") != session_id
        ):
            raise ValueError("invalid Pi session header")
        if value.get("private", False) is not False:
            raise EphemeralSession

    @staticmethod
    def visible_message(value: object) -> dict | None:
        if not isinstance(value, dict) or value.get("type") == "session":
            raise ValueError("invalid Pi session entry")
        if value.get("type") != "message":
            return None
        message = value.get("message")
        if not isinstance(message, dict):
            raise TypeError("invalid Pi message")
        role = message.get("role")
        if role not in ("user", "assistant"):
            return None
        content = message.get("content")
        if isinstance(content, str) and role == "user":
            text = [content]
        elif isinstance(content, list):
            text = []
            for block in content:
                if not isinstance(block, dict) or not isinstance(
                    block.get("type"), str
                ):
                    raise TypeError("invalid Pi content block")
                if block["type"] == "text":
                    if not isinstance(block.get("text"), str):
                        raise ValueError("invalid Pi text block")
                    text.append(block["text"])
        else:
            raise ValueError("invalid Pi message content")
        return {"role": role, "content": text} if text else None

    def discover_replay_sessions(
        self, home: Path, config: dict[str, str]
    ) -> list[ReplaySession | None]:
        root = self.sessions_dir(home, config)
        sessions: list[ReplaySession | None] = []
        for path in sorted(root.glob("--*--/*_*.jsonl")):
            session_id = path.stem.partition("_")[2]
            try:
                session_parts(path, root)
                if not path.is_file() or not safe_id(session_id):
                    raise ValueError("invalid Pi session file")
                self.check_header(first_record(path), session_id)
            except EphemeralSession:
                continue
            except (OSError, ValueError, RuntimeError):
                sessions.append(None)
                continue
            sessions.append(ReplaySession(session_id, path, ""))
        return sessions

    def replay_payload(self, session: ReplaySession) -> dict[str, Any]:
        return {
            "session_id": session.session_id,
            "transcript_path": str(session.transcript),
            "hook_event_name": "ManualReplay",
        }

    def discover_captures(self, context: CaptureContext) -> list[CaptureSource]:
        return [
            capture
            for session in self.discover_replay_sessions(
                context.home, dict(context.config)
            )
            if session is not None
            for capture in self.captures_from_event(
                self.replay_payload(session), context
            )
        ]


PROVIDERS: dict[str, ProviderAdapter] = {
    "pi": PiProvider(),
    "claude": ClaudeProvider(),
    "codex": CodexProvider(),
}


def get_provider(name: str) -> ProviderAdapter:
    try:
        return PROVIDERS[name]
    except KeyError as error:
        raise ValueError(f"unsupported provider: {name}") from error
