from __future__ import annotations

import json
from collections.abc import Mapping
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
    archive_on_redaction_failure: bool

    def hook_contract(self, event: str) -> HookContract: ...

    def captures_from_event(
        self, payload: Mapping[str, object], context: CaptureContext
    ) -> list[CaptureSource]: ...

    def discover_captures(self, context: CaptureContext) -> list[CaptureSource]: ...

    def sessions_dir(self, home: Path, config: dict[str, str]) -> Path: ...

    def discover_replay_sessions(
        self, home: Path, config: dict[str, str]
    ) -> list[ReplaySession]: ...

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
    except OSError:
        pass
    return objects


def first_record(path: Path) -> dict[str, Any]:
    try:
        with path.open(encoding="utf-8") as handle:
            value = json.loads(handle.readline())
    except (OSError, json.JSONDecodeError):
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


class ClaudeProvider:
    name = "claude"
    hook_events = ("session-end",)
    archive_on_redaction_failure = True

    def hook_contract(self, event: str) -> HookContract:
        if event == "session-end":
            return HookContract(mode="payload", returns_continue=False)
        raise ValueError(f"unsupported Claude hook event: {event}")

    def sessions_dir(self, home: Path, config: dict[str, str]) -> Path:
        return home / ".claude/projects"

    def captures_from_event(
        self, payload: Mapping[str, object], context: CaptureContext
    ) -> list[CaptureSource]:
        session_id = payload.get("session_id")
        transcript_raw = payload.get("transcript_path")
        if not isinstance(session_id, str) or not isinstance(transcript_raw, str):
            raise TypeError("Claude payload missing session_id or transcript_path")
        transcript = Path(transcript_raw)
        if not transcript.is_file():
            raise ValueError(f"Claude transcript missing: {transcript}")
        if "subagents" in transcript.parts:
            parent_id = first_jsonl_value(
                transcript, lambda item: item.get("sessionId")
            )
            return [self._subagent_capture(transcript, parent_id)]

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
        if subagents.is_dir():
            for child in sorted(subagents.glob("agent-*.jsonl")):
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
        if not agent_id:
            raise ValueError(
                f"Claude subagent transcript missing agent id: {transcript}"
            )
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
        transcripts = set(projects_dir.rglob("*.jsonl"))
        direct = {path for path in transcripts if "subagents" not in path.parts}
        orphans: set[Path] = set()
        for path in transcripts:
            if path.parent.name != "subagents" or not path.name.startswith("agent-"):
                continue
            parent_transcript = path.parent.parent.with_suffix(".jsonl")
            if not parent_transcript.is_file():
                orphans.add(path)

        sessions: list[ReplaySession] = []
        for transcript in sorted(direct | orphans):
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
    archive_on_redaction_failure = True

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


PROVIDERS: dict[str, ProviderAdapter] = {
    "claude": ClaudeProvider(),
    "codex": CodexProvider(),
}


def get_provider(name: str) -> ProviderAdapter:
    try:
        return PROVIDERS[name]
    except KeyError as error:
        raise ValueError(f"unsupported provider: {name}") from error
