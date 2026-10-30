#!/usr/bin/env bash
set -euo pipefail

activation=$1
shift
expected_arguments="install --apply"
for target in "$@"; do
  expected_arguments+=" --target $target"
done

temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT

check_activation() {
  local mode=$1 expected_status=$2 actual_status=0
  (
    export HOME="$temporary/home"
    export DRY_RUN=""
    if [ "$mode" = dry-run ]; then
      export DRY_RUN=1
    fi
    run() {
      printf '%s\n' called >> "$temporary/$mode.calls"
      if [ "${2#XDG_STATE_HOME=}" = "$2" ] || [ "${*:4}" != "$expected_arguments" ]; then
        printf '%s\n' "Unexpected installer invocation: $*" >&2
        return 99
      fi
      # Intercept the absolute installer command so no install or network request occurs.
      if [ -n "$DRY_RUN" ] || [ "$mode" = success ]; then
        return 0
      fi
      return 42
    }
    verboseEcho() { printf '%s\n' "$*"; }
    errorEcho() { printf '%s\n' "$*" >&2; }
    source "$activation"
  ) > "$temporary/$mode.stdout" 2> "$temporary/$mode.stderr" || actual_status=$?
  if [ "$actual_status" -ne "$expected_status" ]; then
    cat "$temporary/$mode.stderr" >&2
    printf 'Expected %s exit %s, received %s\n' "$mode" "$expected_status" "$actual_status" >&2
    exit 1
  fi
  test "$(cat "$temporary/$mode.calls")" = called
}

check_activation success 0
check_activation dry-run 0
check_activation failure 42
rg -q 'Agent skills installation failed' "$temporary/failure.stderr"
