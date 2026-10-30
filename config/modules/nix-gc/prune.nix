# Shared retention policy for the Home Manager and NixOS collectors.
#
# nix-collect-garbage understands an age cutoff but not a generation count, so
# retention is applied per profile with nix-env before the store sweep. Running
# both forms keeps a generation only while it is newer than maxAge *and* among
# the newest keepGenerations. nix-env always refuses to delete the current one.
{
  pkgs,
  nixPackage,
  keepGenerations,
  maxAge,
  profilesDir,
}:

pkgs.writeShellApplication {
  name = "nix-gc-prune";
  runtimeInputs = [ nixPackage ];
  text = ''
    profiles=${profilesDir}

    # The per-user patterns stay unmatched outside /nix/var/nix/profiles; an
    # unexpanded glob is not a symlink, so the guard below drops it.
    for profile in "$profiles"/* "$profiles"/per-user/*/*; do
      # Generation links sit beside the profile symlinks they belong to and are
      # not themselves valid --profile targets.
      case "$profile" in
        *-link) continue ;;
      esac
      [ -L "$profile" ] || continue
      [ -e "$profile" ] || continue

      echo "Pruning $profile"
      nix-env --profile "$profile" --delete-generations "+${toString keepGenerations}"
      nix-env --profile "$profile" --delete-generations "${maxAge}"
    done

    echo "Collecting garbage"
    nix-collect-garbage
  '';
}
