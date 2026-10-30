#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
SUBJECT="${SCRIPT_DIR}/tmux-session-manager.sh"
TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/tmux-session-manager-test.XXXXXX")"
FAKE_TMUX="${TEST_ROOT}/tmux"
FAKE_FZF="${TEST_ROOT}/fzf"
TMUX_LOG="${TEST_ROOT}/tmux.log"
FZF_LOG="${TEST_ROOT}/fzf.log"
FZF_INPUT="${TEST_ROOT}/fzf-input"
FZF_OUTPUT_DIR="${TEST_ROOT}/fzf-outputs"
FZF_COUNT="${TEST_ROOT}/fzf-count"
SESSIONS_FILE="${TEST_ROOT}/sessions"

trap 'rm -rf "${TEST_ROOT}"' EXIT

cat >"${FAKE_TMUX}" <<'FAKE_TMUX'
#!/usr/bin/env bash

set -euo pipefail

: "${TMUX_TEST_LOG:?}"
: "${TMUX_TEST_SESSIONS:?}"

if [ -n "${TMUX_TEST_EXPECT_TMUX_TMPDIR:-}" ] && [ "${TMUX_TMPDIR:-}" != "${TMUX_TEST_EXPECT_TMUX_TMPDIR}" ]; then
  printf 'unexpected TMUX_TMPDIR: %s\n' "${TMUX_TMPDIR:-}" >&2
  exit 1
fi

printf 'args:%s\n' "$*" >>"${TMUX_TEST_LOG}"

case "${1:-} ${2:-}" in
  'list-sessions -F')
    expected_format=$'#{session_id}\t#{session_name}\t#{session_windows} windows'
    if [ "${3:-}" != "${expected_format}" ]; then
      printf 'unexpected tmux list format: %s\n' "${3:-}" >&2
      exit 2
    fi

    if [ -s "${TMUX_TEST_SESSIONS}" ]; then
      cat "${TMUX_TEST_SESSIONS}"
    else
      exit 1
    fi
    ;;
  'attach-session -t')
    printf 'attached:%s\n' "${3:-}" >>"${TMUX_TEST_LOG}"
    ;;
  'new-session -s')
    printf 'new:%s\n' "${3:-}" >>"${TMUX_TEST_LOG}"
    ;;
  'rename-session -t')
    printf 'renamed:%s:%s\n' "${3:-}" "${4:-}" >>"${TMUX_TEST_LOG}"
    ;;
  'kill-session -t')
    printf 'killed:%s\n' "${3:-}" >>"${TMUX_TEST_LOG}"
    ;;
  *)
    printf 'unexpected tmux args: %s\n' "$*" >&2
    exit 2
    ;;
esac
FAKE_TMUX

cat >"${FAKE_FZF}" <<'FAKE_FZF'
#!/usr/bin/env bash

set -euo pipefail

: "${TMUX_TEST_FZF_INPUT:?}"
: "${TMUX_TEST_FZF_LOG:?}"
: "${TMUX_TEST_FZF_OUTPUT_DIR:?}"
: "${TMUX_TEST_FZF_COUNT:?}"

has_arg() {
  local expected="${1}"
  local arg

  shift
  for arg in "$@"; do
    if [ "${arg}" = "${expected}" ]; then
      return 0
    fi
  done

  return 1
}

has_delimiter_arg() {
  local args=("$@")
  local index

  for index in "${!args[@]}"; do
    case "${args[${index}]}" in
      $'--delimiter=\t')
        return 0
        ;;
      '--delimiter')
        if [ "${args[$((index + 1))]:-}" = $'\t' ]; then
          return 0
        fi
        ;;
    esac
  done

  return 1
}

has_arg_prefix() {
  local prefix="${1}"
  local arg

  shift
  for arg in "$@"; do
    if [[ "${arg}" == "${prefix}"* ]]; then
      return 0
    fi
  done

  return 1
}

if has_arg '--prompt=tmux> ' "$@"; then
  if ! has_arg '--print-query' "$@"; then
    printf 'missing required fzf arg: --print-query\n' >&2
    exit 2
  fi

  if ! has_arg '--expect=ctrl-r,ctrl-d' "$@"; then
    printf 'missing required fzf arg: --expect=ctrl-r,ctrl-d\n' >&2
    exit 2
  fi

  if ! has_delimiter_arg "$@"; then
    printf 'missing required fzf tab delimiter\n' >&2
    exit 2
  fi

  if ! has_arg '--with-nth=2,3' "$@"; then
    printf 'missing required fzf arg: --with-nth=2,3\n' >&2
    exit 2
  fi

  if ! has_arg '--bind=enter:print(enter)+accept-or-print-query' "$@"; then
    printf 'missing required fzf arg: --bind=enter:print(enter)+accept-or-print-query\n' >&2
    exit 2
  fi

  if ! has_arg '--header=enter: attach | type new name + enter: create | ctrl-r: rename | ctrl-d: delete' "$@"; then
    printf 'missing required fzf header explaining query creation\n' >&2
    exit 2
  fi
elif has_arg_prefix '--prompt=rename ' "$@"; then
  if ! has_arg '--print-query' "$@"; then
    printf 'missing required rename fzf arg: --print-query\n' >&2
    exit 2
  fi

  if ! has_arg '--bind=enter:print()+accept-or-print-query' "$@"; then
    printf 'missing required rename fzf arg: --bind=enter:print()+accept-or-print-query\n' >&2
    exit 2
  fi

  if ! has_arg '--header=enter: rename | esc: cancel' "$@"; then
    printf 'missing required rename fzf header\n' >&2
    exit 2
  fi
elif has_arg_prefix '--prompt=delete ' "$@"; then
  if ! has_delimiter_arg "$@"; then
    printf 'missing required delete fzf tab delimiter\n' >&2
    exit 2
  fi

  if ! has_arg '--with-nth=2' "$@"; then
    printf 'missing required delete fzf arg: --with-nth=2\n' >&2
    exit 2
  fi

  if ! has_arg '--header=enter: confirm | esc: cancel' "$@"; then
    printf 'missing required delete fzf header\n' >&2
    exit 2
  fi
else
  printf 'unexpected fzf prompt args: %s\n' "$*" >&2
  exit 2
fi

printf 'fzf\n' >>"${TMUX_TEST_FZF_LOG}"
cat >"${TMUX_TEST_FZF_INPUT}"

call_count=0
if [ -s "${TMUX_TEST_FZF_COUNT}" ]; then
  call_count="$(cat "${TMUX_TEST_FZF_COUNT}")"
fi
call_count=$((call_count + 1))
printf '%s\n' "${call_count}" >"${TMUX_TEST_FZF_COUNT}"

output_file="${TMUX_TEST_FZF_OUTPUT_DIR}/${call_count}"
if [ ! -e "${output_file}" ]; then
  exit 130
fi

cat "${output_file}"
FAKE_FZF

chmod +x "${FAKE_TMUX}" "${FAKE_FZF}"

reset_files() {
  : >"${TMUX_LOG}"
  : >"${FZF_LOG}"
  : >"${FZF_INPUT}"
  : >"${SESSIONS_FILE}"
  : >"${FZF_COUNT}"
  rm -rf "${FZF_OUTPUT_DIR}"
  mkdir -p "${FZF_OUTPUT_DIR}"
}

prepare_fzf_outputs() {
  local index=1
  local output

  rm -rf "${FZF_OUTPUT_DIR}"
  mkdir -p "${FZF_OUTPUT_DIR}"

  for output in "$@"; do
    printf '%s' "${output}" >"${FZF_OUTPUT_DIR}/${index}"
    index=$((index + 1))
  done
}

run_manager() {
  local stdin="${1:-}"
  shift || true

  prepare_fzf_outputs "$@"

  TMUX_TEST_LOG="${TMUX_LOG}" \
    TMUX_TEST_SESSIONS="${SESSIONS_FILE}" \
    TMUX_TEST_FZF_INPUT="${FZF_INPUT}" \
    TMUX_TEST_FZF_LOG="${FZF_LOG}" \
    TMUX_TEST_FZF_OUTPUT_DIR="${FZF_OUTPUT_DIR}" \
    TMUX_TEST_FZF_COUNT="${FZF_COUNT}" \
    bash "${SUBJECT}" "${FAKE_TMUX}" "${FAKE_FZF}" <<<"${stdin}"
}

run_manager_without_tmux_tmpdir() {
  local stdin="${1:-}"
  local runtime_dir="${TEST_ROOT}/runtime"
  shift || true

  mkdir -p "${runtime_dir}"
  prepare_fzf_outputs "$@"

  env -u TMUX_TMPDIR \
    XDG_RUNTIME_DIR="${runtime_dir}" \
    TMUX_TEST_EXPECT_TMUX_TMPDIR="${runtime_dir}" \
    TMUX_TEST_LOG="${TMUX_LOG}" \
    TMUX_TEST_SESSIONS="${SESSIONS_FILE}" \
    TMUX_TEST_FZF_INPUT="${FZF_INPUT}" \
    TMUX_TEST_FZF_LOG="${FZF_LOG}" \
    TMUX_TEST_FZF_OUTPUT_DIR="${FZF_OUTPUT_DIR}" \
    TMUX_TEST_FZF_COUNT="${FZF_COUNT}" \
    bash "${SUBJECT}" "${FAKE_TMUX}" "${FAKE_FZF}" <<<"${stdin}"
}

assert_log_equals() {
  local expected="${1}"
  local actual

  actual="$(cat "${TMUX_LOG}")"
  if [ "${actual}" = "${expected}" ]; then
    return 0
  fi

  printf 'Unexpected tmux log\n' >&2
  printf 'Expected log:\n%s\n' "${expected}" >&2
  printf 'Actual log:\n' >&2
  cat "${TMUX_LOG}" >&2
  return 1
}

assert_file_equals() {
  local file="${1}"
  local expected="${2}"
  local actual

  actual="$(cat "${file}")"
  if [ "${actual}" != "${expected}" ]; then
    printf 'Unexpected file content for %s\n' "${file}" >&2
    printf 'Expected:\n%s\n' "${expected}" >&2
    printf 'Actual:\n%s\n' "${actual}" >&2
    return 1
  fi
}

test_attach_selected_session() {
  reset_files
  printf '@1\twork\t2 windows\n@2\tops\t1 windows\n' >"${SESSIONS_FILE}"

  run_manager '' $'\n\nenter\n@2\tops\t1 windows\n'

  assert_log_equals $'args:list-sessions -F #{session_id}\t#{session_name}\t#{session_windows} windows\nargs:attach-session -t @2\nattached:@2'
  assert_file_equals "${FZF_LOG}" 'fzf'
  assert_file_equals "${FZF_INPUT}" $'@1\twork\t2 windows\n@2\tops\t1 windows'
}

test_rename_selected_session() {
  reset_files
  printf '@1\twork\t2 windows\n' >"${SESSIONS_FILE}"

  run_manager '' $'\nctrl-r\n@1\twork\t2 windows\n' $'renamed\n\n'

  assert_log_equals $'args:list-sessions -F #{session_id}\t#{session_name}\t#{session_windows} windows\nargs:rename-session -t @1 renamed\nrenamed:@1:renamed\nargs:list-sessions -F #{session_id}\t#{session_name}\t#{session_windows} windows'
  assert_file_equals "${FZF_LOG}" $'fzf\nfzf\nfzf'
}

test_delete_selected_session() {
  reset_files
  printf '@1\twork\t2 windows\n' >"${SESSIONS_FILE}"

  run_manager '' $'\nctrl-d\n@1\twork\t2 windows\n' $'yes\tDelete\n'

  assert_log_equals $'args:list-sessions -F #{session_id}\t#{session_name}\t#{session_windows} windows\nargs:kill-session -t @1\nkilled:@1\nargs:list-sessions -F #{session_id}\t#{session_name}\t#{session_windows} windows'
  assert_file_equals "${FZF_LOG}" $'fzf\nfzf\nfzf'
}

test_delete_cancel_keeps_selected_session() {
  reset_files
  printf '@1\twork\t2 windows\n' >"${SESSIONS_FILE}"

  run_manager '' $'\nctrl-d\n@1\twork\t2 windows\n' $'cancel\tCancel\n'

  assert_log_equals $'args:list-sessions -F #{session_id}\t#{session_name}\t#{session_windows} windows\nargs:list-sessions -F #{session_id}\t#{session_name}\t#{session_windows} windows'
  assert_file_equals "${FZF_LOG}" $'fzf\nfzf\nfzf'
}

test_enter_query_creates_session_when_no_row_matches() {
  reset_files
  printf '@1\twork\t2 windows\n' >"${SESSIONS_FILE}"

  run_manager '' $'fresh\n\n'

  assert_log_equals $'args:list-sessions -F #{session_id}\t#{session_name}\t#{session_windows} windows\nargs:new-session -s fresh\nnew:fresh'
  assert_file_equals "${FZF_LOG}" 'fzf'
}

test_missing_tmux_tmpdir_uses_xdg_runtime_dir_for_existing_sessions() {
  reset_files
  printf '@1\twork\t2 windows\n@2\tops\t1 windows\n' >"${SESSIONS_FILE}"

  run_manager_without_tmux_tmpdir '' $'\n\nenter\n@2\tops\t1 windows\n'

  assert_log_equals $'args:list-sessions -F #{session_id}\t#{session_name}\t#{session_windows} windows\nargs:attach-session -t @2\nattached:@2'
  assert_file_equals "${FZF_LOG}" 'fzf'
  assert_file_equals "${FZF_INPUT}" $'@1\twork\t2 windows\n@2\tops\t1 windows'
}

test_no_sessions_prompts_for_new_session() {
  reset_files

  run_manager $'fresh\n' ''

  assert_log_equals $'args:list-sessions -F #{session_id}\t#{session_name}\t#{session_windows} windows\nargs:new-session -s fresh\nnew:fresh'
  assert_file_equals "${FZF_LOG}" ''
  assert_file_equals "${FZF_INPUT}" ''
}

run_test() {
  local name="${1}"

  printf 'running %s\n' "${name}"
  "${name}"
}

run_test test_attach_selected_session
run_test test_rename_selected_session
run_test test_delete_selected_session
run_test test_delete_cancel_keeps_selected_session
run_test test_enter_query_creates_session_when_no_row_matches
run_test test_missing_tmux_tmpdir_uses_xdg_runtime_dir_for_existing_sessions
run_test test_no_sessions_prompts_for_new_session

printf 'tmux-session-manager tests passed\n'
