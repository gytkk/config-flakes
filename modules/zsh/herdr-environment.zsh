# A Core daemon started inside the TUI can pass its old pane identity to new shells.
if [[ -n ${HC_TERMINAL_ID:-} && -n ${HERDR_PANE_ID:-} && $(ps -o comm= -p "$PPID") == (*/|)herdr-core ]]; then
  unset HERDR_ENV HERDR_PANE_ID HERDR_SOCKET_PATH HERDR_WORKSPACE_ID HERDR_TAB_ID HERDR_BIN_PATH
fi
