# Claude session mining

`modules.claudeSessionMining` schedules an externally managed session-mining runner with a macOS launchd agent. It does not install the runner or manage Claude Code hooks and transcript capture.

The module is disabled by default. Leave it unconfigured on hosts that do not have the external runner checkout.

Enable it only in a selected Darwin profile or host:

```nix
modules.claudeSessionMining = {
  enable = true;
  runnerPath = "${config.home.homeDirectory}/workspace/gytkk-space/automation/mine-sessions.sh";
};
```

The runner path must be absolute. Activation creates `~/Library/Logs/claude-mining` and writes a stderr warning if the runner is missing or not executable, but it does not block Home Manager activation. The agent runs daily at 06:00 with the configured runner as its fixed `ProgramArguments` path.
