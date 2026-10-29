# Exclude old commit messages before autosuggestions can cache a partial match.
ZSH_AUTOSUGGEST_HISTORY_IGNORE='gc -m*'
ZSH_AUTOSUGGEST_STRATEGY=(git_commit history)

_zsh_autosuggest_strategy_git_commit() {
  emulate -L zsh
  local message
  suggestion=''
  case "$1" in
    'gc -m'|'gc -m '|'gc -m "'|"gc -m '") ;;
    *) return ;;
  esac

  # The async plugin cancels this debounce when the user continues typing.
  sleep 0.3
  message=$(git-commit-message) || return
  [[ -n "$message" ]] || return
  if [[ "$1" == 'gc -m "' ]]; then
    suggestion="gc -m ${(qqq)message}"
  else
    suggestion="gc -m ${(qq)message}"
  fi
}
