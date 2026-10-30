if [ "${HERDR_ENV:-}" = "1" ]; then
  # Shared daemons retain the first pane's environment across Codex sessions.
  codex_daemon_flag=--no-daemon
  for codex_arg in "$@"; do
    case "$codex_arg" in
      --) break ;;
      --no-daemon) codex_daemon_flag=; break ;;
    esac
  done
  if [ -n "$codex_daemon_flag" ]; then
    set -- --no-daemon "$@"
  fi
fi
exec @codex@ "$@"
