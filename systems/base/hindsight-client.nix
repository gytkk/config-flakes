{
  config,
  flakeDirectory,
  lib,
  pkgs,
  osConfig ? null,
  isWSL ? false,
  ...
}:
let
  localServer = osConfig != null && osConfig.networking.hostName == "pylv-sepia";
  publicClient = !isWSL && !localServer;
  example = lib.importJSON ../../modules/pi/files/hindsight-sepia.example.json;
  seed = pkgs.writeText "hindsight-client-defaults.json" (
    builtins.toJSON (
      example
      // {
        harnesses.pi = example.harnesses.pi // {
          apiUrl = if localServer then "http://127.0.0.1:8888" else example.harnesses.pi.apiUrl;
          mapPathToBank = {
            ${flakeDirectory} = "config-flakes";
          };
        };
      }
    )
  );
  secrets = (if osConfig == null then config else osConfig).age.secrets;
in
{
  age.secrets = lib.mkIf (osConfig == null) {
    hindsight-cloudflare-access = lib.mkIf publicClient {
      file = ../../secrets/hindsight-cloudflare-access.age;
      mode = "0400";
    };
    hindsight-client-token = {
      file = ../../secrets/hindsight-client-token.age;
      mode = "0400";
    };
  };
  home.activation.piHindsight = lib.mkIf config.modules.pi.enable (
    lib.hm.dag.entryAfter [ "writeBoundary" ] ''
      run ${pkgs.bash}/bin/bash ${../../modules/pi/files/seed-hindsight.sh} ${seed}
    ''
  );
  modules.pi.hindsightTokenFile = secrets.hindsight-client-token.path;
  modules.pi.hindsightLan = isWSL;
  modules.pi.hindsightAccessFile = lib.mkIf publicClient secrets.hindsight-cloudflare-access.path;
}
