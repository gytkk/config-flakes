#!/usr/bin/env bash

set -u

agent="${1:-}"
state="${2:-}"
target_window="${3:-}"

usage() {
  printf 'usage: tmux-agent-status <agent> <running|waiting|done|failed|clear> [window_id]\n' >&2
}

valid_window_id() {
  [[ "$1" =~ ^@[0-9]+$ ]]
}

cache_base="${XDG_CACHE_HOME:-}"
if [ -z "${cache_base}" ]; then
  if [ -z "${HOME:-}" ]; then
    exit 1
  fi
  cache_base="${HOME}/.cache"
fi
cache_dir="${cache_base}/tmux-agent-status"

if [ -z "${agent}" ] || [ -z "${state}" ]; then
  usage
  exit 2
fi

case "${state}" in
  running | waiting | done | failed | clear) ;;
  *)
    usage
    exit 2
    ;;
esac

if [ -z "${target_window}" ] && [ -n "${TMUX_AGENT_WINDOW_ID:-}" ]; then
  target_window="${TMUX_AGENT_WINDOW_ID}"
fi

if [ -z "${target_window}" ] && [ -n "${TMUX_PANE:-}" ] && command -v tmux >/dev/null 2>&1; then
  target_window="$(tmux display-message -p -t "${TMUX_PANE}" '#{window_id}' 2>/dev/null || true)"
fi

if [ -z "${target_window}" ] && [ -n "${TMUX:-}" ] && command -v tmux >/dev/null 2>&1; then
  target_window="$(tmux display-message -p '#{window_id}' 2>/dev/null || true)"
fi

if [ -z "${target_window}" ]; then
  # Hooks can run outside tmux during tests or non-interactive agent runs.
  exit 0
fi

if ! valid_window_id "${target_window}"; then
  usage
  exit 2
fi

mkdir -p "${cache_dir}" || exit 1
status_file="${cache_dir}/${target_window}"

if [ "${state}" = "clear" ]; then
  rm -f "${status_file}"
else
  now="$(date +%s)"
  printf '%s\t%s\t%s\t%s\n' "${state}" "${agent}" "${now}" "${agent}" > "${status_file}"
fi

if [ -n "${TMUX:-}" ] && command -v tmux >/dev/null 2>&1; then
  tmux refresh-client -S 2>/dev/null || true
fi
