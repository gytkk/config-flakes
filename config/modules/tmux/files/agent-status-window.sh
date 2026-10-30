#!/usr/bin/env bash

set -u

window_id="${1:-}"

valid_window_id() {
  [[ "$1" =~ ^@[0-9]+$ ]]
}

[ -n "${window_id}" ] || exit 0
valid_window_id "${window_id}" || exit 0

cache_base="${XDG_CACHE_HOME:-}"
if [ -z "${cache_base}" ]; then
  if [ -z "${HOME:-}" ]; then
    exit 0
  fi
  cache_base="${HOME}/.cache"
fi
cache_dir="${cache_base}/tmux-agent-status"
status_file="${cache_dir}/${window_id}"

[ -r "${status_file}" ] || exit 0

IFS=$'\t' read -r state agent updated label < "${status_file}" || exit 0

case "${updated}" in
  '' | *[!0-9]*) exit 0 ;;
esac

now="$(date +%s)"
age=$((now - updated))

case "${state}" in
  running | waiting)
    ttl=$((12 * 60 * 60))
    ;;
  done)
    ttl=$((30 * 60))
    ;;
  failed)
    ttl=$((2 * 60 * 60))
    ;;
  *)
    exit 0
    ;;
esac

if [ "${age}" -gt "${ttl}" ]; then
  rm -f "${status_file}" 2>/dev/null || true
  exit 0
fi

case "${state}" in
  running)
    printf '#[fg=blue]●#[default]'
    ;;
  waiting)
    printf '#[fg=yellow]?#[default]'
    ;;
  done)
    printf '#[fg=green]✓#[default]'
    ;;
  failed)
    printf '#[fg=red]×#[default]'
    ;;
esac
