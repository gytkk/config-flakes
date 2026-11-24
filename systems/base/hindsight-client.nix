{
  config,
  lib,
  osConfig ? null,
  isWSL ? false,
  ...
}:
let
  publicClient = !isWSL && (osConfig == null || osConfig.networking.hostName != "pylv-sepia");
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
  modules.pi.hindsightTokenFile = secrets.hindsight-client-token.path;
  modules.pi.hindsightLan = isWSL;
  modules.pi.hindsightAccessFile = lib.mkIf publicClient secrets.hindsight-cloudflare-access.path;
}
