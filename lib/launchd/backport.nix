{ lib, source }:
let
  replaceExactlyOnce =
    before: after: text:
    let
      pieces = lib.splitString before text;
    in
    if builtins.length pieces != 2 then
      throw "Home Manager launchd backport anchor did not occur exactly once"
    else
      builtins.concatStringsSep after pieces;
in
replaceExactlyOnce (builtins.readFile ./activation.before) (builtins.readFile ./activation.after)
  source
