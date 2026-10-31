# Keep the validation lazy so malformed entries fail before builder dependencies.
context: config: value:
let
  requiredFields = [
    "system"
    "username"
    "homeDirectory"
    "profile"
  ];
  missingFields = builtins.filter (field: !(builtins.hasAttr field config)) requiredFields;
in
if missingFields != [ ] then
  throw "Missing required fields for ${context}: ${builtins.toString missingFields}"
else
  value
