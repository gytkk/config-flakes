zmodload zsh/datetime
autoload -Uz add-zsh-hook

_nix_command_timing_start() {
  local -a words
  words=("${(@z)2}")
  unset _nix_command_started _nix_command_name
  [[ ${words[-1]-} == '&' || ${words[-1]-} == '&!' ]] && return 0

  # Inspect the expanded command without wrapping sudo or changing its arguments.
  while (( $#words )); do
    case "${words[1]:t}" in
      command|time|noglob|nocorrect) shift words ;;
      sudo)
        shift words
        while (( $#words )); do
          case "${words[1]}" in
            --) shift words; break ;;
            -u|-g|-h|-p|-C|-T|-R|-D|--user|--group|--host|--prompt|--close-from|--command-timeout|--chroot|--chdir)
              (( $#words >= 2 )) || return 0
              shift 2 words
              ;;
            -*) shift words ;;
            *) break ;;
          esac
        done
        ;;
      home-manager|nixos-rebuild)
        typeset -g _nix_command_name="${words[1]:t}"
        typeset -gF _nix_command_started=$EPOCHREALTIME
        break
        ;;
      *) break ;;
    esac
  done
  return 0
}

_nix_command_timing_finish() {
  if [[ -n ${_nix_command_started-} ]]; then
    local -i elapsed=$(( EPOCHREALTIME - _nix_command_started + 0.5 ))
    (( elapsed < 0 )) && elapsed=0
    printf '\n[%s] 소요 시간: %d분 %d초\n' \
      "$_nix_command_name" $(( elapsed / 60 )) $(( elapsed % 60 )) >&2
    unset _nix_command_started _nix_command_name
  fi
  return 0
}

add-zsh-hook preexec _nix_command_timing_start
add-zsh-hook precmd _nix_command_timing_finish
