{
  config,
  lib,
  osConfig ? null,
  pkgs,
  ...
}:
let
  cfg = config.modules.openaiApi;
  usesSystemAgenix = osConfig != null;
  secretPath =
    if usesSystemAgenix then
      osConfig.age.secrets.openai-api-key.path
    else
      config.age.secrets.openai-api-key.path;
  shellSecretPath = lib.concatMapStringsSep ''"''${XDG_RUNTIME_DIR}"'' lib.escapeShellArg (
    lib.splitString "\${XDG_RUNTIME_DIR}" secretPath
  );
in
{
  options.modules.openaiApi.enable = lib.mkEnableOption "OpenAI API access with the agenix key";

  config = lib.mkIf cfg.enable {
    age.secrets = lib.mkIf (!usesSystemAgenix) {
      openai-api-key = {
        file = ../../secrets/openai-api-key.age;
        mode = "0400";
      };
    };

    home.packages = [
      (pkgs.writeShellApplication {
        name = "with-openai";
        runtimeInputs = [ pkgs.coreutils ];
        text = builtins.replaceStrings [ "@secretPath@" ] [ shellSecretPath ] (
          builtins.readFile ./with-openai.sh
        );
      })
    ];
  };
}
