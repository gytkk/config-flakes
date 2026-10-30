#!/usr/bin/env bash

set -u

agent="${1:-}"
if [ -z "${agent}" ] || [ "$#" -lt 2 ]; then
  printf 'usage: agent-run <agent> <command> [args...]\n' >&2
  exit 2
fi
shift

status_bin="${TMUX_AGENT_STATUS_BIN:-tmux-agent-status}"

"${status_bin}" "${agent}" running 2>/dev/null || true
"$@"
code=$?

if [ "${code}" -eq 0 ]; then
  "${status_bin}" "${agent}" done 2>/dev/null || true
else
  "${status_bin}" "${agent}" failed 2>/dev/null || true
fi

exit "${code}"
