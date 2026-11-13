{ lib, source }:
let
  marker = "  options.age = {\n";
  parts = lib.splitString marker source;
in
if builtins.length parts != 2 then
  throw "agenix Home Manager module changed: review the mountingScript interface patch"
else
  lib.concatStringsSep ''
    options.age = {
      mountingScript = mkOption {
        type = types.str;
        readOnly = true;
        internal = true;
        default = mountingScript;
        description = "Generated agenix mount command used by platform integration.";
      };
  '' parts
