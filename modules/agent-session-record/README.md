# Agent session recording

[default.nix](default.nix) installs the `agent-session-record` CLI and its Home Manager configuration. Hooks capture transcripts into a local queue; successful uploads update the session state and capture receipt. Failed uploads leave the snapshot queued for retry.

The runtime modules own capture triggers: Claude uses `SessionEnd`, Codex uses `SessionStart` and `Stop`, and Pi uses `session_shutdown` for persisted sessions. The recorder module does not add hooks itself. The common profile enables recording, while the Devsisters profile explicitly disables it. When disabled, Home Manager installs a successful no-op entrypoint so the shared runtime hooks do not record or upload; existing state and previously uploaded sessions are not removed.

Pi capture and `agent-session-record replay pi` accept v3 JSONL files only in the default Pi session store. The filename and session header must match the session ID. The recorder keeps only user and assistant text, dropping images, thinking, tool calls and results, extension entries, and other message fields before redaction. In-memory sessions, custom stores outside the default directory, and headers marked private are not captured. Manual Pi replay continues past invalid inputs, reports the skipped count without transcript content, and exits unsuccessfully on partial failure. The archive still contains the common capture manifest, including session ID, source path, host and timestamps.

Claude validates transcript paths under the configured projects directory and briefly retries a missing file. On Linux the synchronous hook checks its immediate producer before detaching the worker. It recognizes the native Nix `claude-code` executable (including `.claude-wrapped`), directly or through the configured hook shell command, and an exact `--no-session-persistence` option before any prompt or unrecognized argument. It never logs arguments. For a verified opt-out, the hook does not archive the transcript even if it already exists. Invalid nonempty paths still fail validation. An empty transcript path also indicates no transcript. Unknown producers, unsupported platforms, and ordinary missing files with nonempty paths retain the failure diagnostic; `reason: other` alone does not distinguish them.

Transcripts must pass both redaction and privacy checks before they can be queued or uploaded, including through the local archive shortcut. Invalid JSON, invalid encoding, or detected residual sensitive content fails closed: the original source stays untouched locally, temporary partial output is discarded, and only a content-free warning and capture-attempt status are recorded. No raw fallback is uploaded. Previously queued snapshots marked as failed by either check are retained in place for manual review, never uploaded, migrated, or deleted automatically—even when a newer capture has already been stored. A successfully sanitized empty transcript is still archived but excluded from derivation.

Remote uploads share a 300-second deadline across directory preparation, both rsync transfers, and final publication. Each transport runs in its own process group. Cleanup kills remaining local group members and waits up to five seconds for the launcher, including after timeout or interruption. This does not roll back remote writes or stop a process that deliberately creates another session or process group.

Directory locks keep their PID and creation timestamp, and add a unique ownership token. A live PID never expires by age, including when permissions prevent probing it. Dead owners can be reclaimed immediately; incomplete locks without a valid PID expire after 300 seconds. Lock creation, reclamation, and release use a short file lock on `locks/.guard` to prevent competing workers from deleting a replacement lock. A worker removes a directory lock only when its token still matches. The file lock is released before capture or upload work begins.

Run the offline regression tests from the repository root:

```bash
uv run --no-project -m unittest discover -s modules/agent-session-record/tests
```

Tests use temporary state and local transport fixtures. Actual SSH service behavior, network failures, and Linux execution require separate host verification.
