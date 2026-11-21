#!/usr/bin/env zsh
set -eu

script=${0:A:h:h}/herdr-environment.zsh
export HC_TERMINAL_ID=t3 HERDR_ENV=1 HERDR_PANE_ID=w1:pE HERDR_SOCKET_PATH=/tmp/test-herdr.sock
for parent in herdr-core herdr; do
  (
    function ps() { print -r -- "/test/bin/$parent"; }
    source "$script"
    [[ $HC_TERMINAL_ID == t3 ]]
    if [[ $parent == herdr-core ]]; then
      [[ -z ${HERDR_PANE_ID+x} && -z ${HERDR_ENV+x} && -z ${HERDR_SOCKET_PATH+x} ]]
    else
      [[ $HERDR_PANE_ID == w1:pE && $HERDR_ENV == 1 && -n $HERDR_SOCKET_PATH ]]
    fi
  )
done
