# Remove the previous prompt timer when reloading this file in an existing shell.
autoload -Uz add-zsh-hook
add-zsh-hook -d preexec _nix_command_timing_start
add-zsh-hook -d precmd _nix_command_timing_finish

home-manager() {
  command nix-command-timing --tool home-manager -- home-manager "$@"
}

nixos-rebuild() {
  command nix-command-timing --tool nixos-rebuild -- nixos-rebuild "$@"
}

sudo() {
  local -a words
  words=("$@")
  while (( $#words )); do
    case "${words[1]}" in
      --) shift words; break ;;
      -u|-g|-h|-p|-C|-T|-R|-D|--user|--group|--host|--prompt|--close-from|--command-timeout|--chroot|--chdir)
        (( $#words >= 2 )) || break
        shift 2 words
        ;;
      -*) shift words ;;
      *) break ;;
    esac
  done
  case "${words[1]:t}" in
    home-manager|nixos-rebuild)
      command nix-command-timing --tool "${words[1]:t}" -- sudo "$@"
      ;;
    *) command sudo "$@" ;;
  esac
}
