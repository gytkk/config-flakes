#!/usr/bin/env bash

set -u

tmux_bin="${1:-}"
fzf_bin="${2:-}"
selected_session_id=''
selected_session_name=''
selected_query=''
selected_key=''
selected_row=''

if [ -z "${tmux_bin}" ] || [ ! -x "${tmux_bin}" ]; then
  printf 'tmux session manager: tmux binary is missing or not executable\n' >&2
  exit 1
fi

if [ -z "${fzf_bin}" ] || [ ! -x "${fzf_bin}" ]; then
  printf 'tmux session manager: fzf binary is missing or not executable\n' >&2
  exit 1
fi

set_default_tmux_tmpdir() {
  local runtime_dir

  if [ -n "${TMUX_TMPDIR:-}" ]; then
    return 0
  fi

  if [ -n "${XDG_RUNTIME_DIR:-}" ] && [ -d "${XDG_RUNTIME_DIR}" ]; then
    export TMUX_TMPDIR="${XDG_RUNTIME_DIR%/}"
    return 0
  fi

  runtime_dir="/run/user/$(id -u)"
  if [ -d "${runtime_dir}" ]; then
    export TMUX_TMPDIR="${runtime_dir}"
  fi
}

set_default_tmux_tmpdir

list_session_rows() {
  "${tmux_bin}" list-sessions -F $'#{session_id}\t#{session_name}\t#{session_windows} windows' 2>/dev/null
}

validate_session_name() {
  local name="${1}"

  if [ -z "${name}" ]; then
    printf 'Name cannot be empty.\n' >&2
    return 1
  fi

  if [[ "${name}" == *:* ]]; then
    printf 'Name cannot contain ":".\n' >&2
    return 1
  fi

  return 0
}

prompt_session_name() {
  local prompt="${1}"
  local name

  while true; do
    if ! read -r -p "${prompt}" name; then
      return 1
    fi

    if ! validate_session_name "${name}"; then
      continue
    fi

    printf '%s\n' "${name}"
    return 0
  done
}

parse_selected_row() {
  local row="${1:-}"
  local rest

  if [ -z "${row}" ]; then
    return 1
  fi

  selected_session_id="${row%%$'\t'*}"
  rest="${row#*$'\t'}"
  selected_session_name="${rest%%$'\t'*}"
  [ -n "${selected_session_id}" ]
}

parse_choice() {
  local choice="${1:-}"
  local rest

  selected_query="${choice%%$'\n'*}"
  if [ "${choice}" = "${selected_query}" ]; then
    selected_key=''
    selected_row=''
    return 0
  fi

  rest="${choice#*$'\n'}"
  selected_key="${rest%%$'\n'*}"
  if [ "${rest}" = "${selected_key}" ]; then
    selected_row=''
    return 0
  fi

  selected_row="${rest#*$'\n'}"

  if [ -z "${selected_key}" ]; then
    case "${selected_row}" in
      enter$'\n'*)
        selected_key='enter'
        selected_row="${selected_row#*$'\n'}"
        ;;
      ctrl-r$'\n'*)
        selected_key='ctrl-r'
        selected_row="${selected_row#*$'\n'}"
        ;;
      ctrl-d$'\n'*)
        selected_key='ctrl-d'
        selected_row="${selected_row#*$'\n'}"
        ;;
    esac
  fi
}

choose_action() {
  local rows="${1}"

  printf '%s\n' "${rows}" | "${fzf_bin}" \
    --height=80% \
    --layout=reverse \
    --border \
    --prompt='tmux> ' \
    $'--delimiter=\t' \
    --with-nth=2,3 \
    --print-query \
    '--bind=enter:print(enter)+accept-or-print-query' \
    --header='enter: attach | type new name + enter: create | ctrl-r: rename | ctrl-d: delete' \
    --expect=ctrl-r,ctrl-d
}

choose_rename_name() {
  local session_name="${1}"
  local choice
  local name

  if ! choice="$(printf '' | "${fzf_bin}" \
    --height=40% \
    --layout=reverse \
    --border \
    --prompt="rename ${session_name}> " \
    --print-query \
    '--bind=enter:print()+accept-or-print-query' \
    --header='enter: rename | esc: cancel')"; then
    return 1
  fi

  name="${choice%%$'\n'*}"
  if ! validate_session_name "${name}"; then
    return 1
  fi

  printf '%s\n' "${name}"
}

confirm_delete_session() {
  local session_name="${1}"
  local choice
  local action

  if ! choice="$(printf 'yes\tDelete\ncancel\tCancel\n' | "${fzf_bin}" \
    --height=40% \
    --layout=reverse \
    --border \
    --prompt="delete ${session_name}> " \
    $'--delimiter=\t' \
    --with-nth=2 \
    --header='enter: confirm | esc: cancel')"; then
    return 1
  fi

  action="${choice%%$'\t'*}"
  [ "${action}" = 'yes' ]
}

create_session_with_name() {
  local name="${1}"

  if ! validate_session_name "${name}"; then
    return 0
  fi

  exec "${tmux_bin}" new-session -s "${name}"
}

create_session() {
  local name

  if ! name="$(prompt_session_name 'New session name: ')"; then
    return 0
  fi

  create_session_with_name "${name}"
}

rename_session() {
  local session_id="${1}"
  local session_name="${2}"
  local name

  if ! name="$(choose_rename_name "${session_name}")"; then
    return 0
  fi

  "${tmux_bin}" rename-session -t "${session_id}" "${name}" >/dev/null
}

delete_session() {
  local session_id="${1}"
  local session_name="${2}"

  if ! confirm_delete_session "${session_name}"; then
    return 0
  fi

  "${tmux_bin}" kill-session -t "${session_id}" >/dev/null
}

attach_session() {
  local session_id="${1}"

  exec "${tmux_bin}" attach-session -t "${session_id}"
}

while true; do
  rows="$(list_session_rows || true)"

  if [ -z "${rows}" ]; then
    create_session
    exit 0
  fi

  if ! choice="$(choose_action "${rows}")"; then
    exit 0
  fi

  parse_choice "${choice}"

  case "${selected_key}" in
    enter | '')
      if parse_selected_row "${selected_row}"; then
        attach_session "${selected_session_id}"
      elif [ -n "${selected_query}" ]; then
        create_session_with_name "${selected_query}"
      fi
      ;;
    ctrl-r)
      if parse_selected_row "${selected_row}"; then
        rename_session "${selected_session_id}" "${selected_session_name}"
      fi
      ;;
    ctrl-d)
      if parse_selected_row "${selected_row}"; then
        delete_session "${selected_session_id}" "${selected_session_name}"
      fi
      ;;
  esac
done
