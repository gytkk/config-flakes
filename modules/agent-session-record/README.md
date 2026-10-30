# Agent session recording

[default.nix](default.nix) installs the `agent-session-record` CLI and its Home Manager configuration. Hooks capture transcripts into a local queue; successful uploads update the session state and capture receipt. Failed uploads leave the snapshot queued for retry.

Remote uploads share a 300-second deadline across directory preparation, both rsync transfers, and final publication. Each transport runs in its own process group. Cleanup kills remaining local group members and waits up to five seconds for the launcher, including after timeout or interruption. This does not roll back remote writes or stop a process that deliberately creates another session or process group.

Directory locks keep their PID and creation timestamp, and add a unique ownership token. A live PID never expires by age, including when permissions prevent probing it. Dead owners can be reclaimed immediately; incomplete locks without a valid PID expire after 300 seconds. Lock creation, reclamation, and release use a short file lock on `locks/.guard` to prevent competing workers from deleting a replacement lock. A worker removes a directory lock only when its token still matches. The file lock is released before capture or upload work begins.

Run the offline regression tests from the repository root:

```bash
uv run --no-project -m unittest discover -s modules/agent-session-record/tests
```

Tests use temporary state and local transport fixtures. Actual SSH service behavior, network failures, and Linux execution require separate host verification.
