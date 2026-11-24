{
  config,
  flakeDirectory,
  lib,
  osConfig ? null,
  isWSL ? false,
  ...
}:
let
  localServer = osConfig != null && osConfig.networking.hostName == "pylv-sepia";
  publicClient = !isWSL && !localServer;
  clientFile = if localServer then "hindsight-client-local.json" else "hindsight-client.json";
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
  home.file.".hindsight/coding-agent.json" = lib.mkIf config.modules.pi.enable {
    source = config.lib.file.mkOutOfStoreSymlink "${flakeDirectory}/modules/pi/files/${clientFile}";
  };
  modules.pi.hindsightTokenFile = secrets.hindsight-client-token.path;
  modules.pi.hindsightLan = isWSL;
  modules.pi.hindsightAccessFile = lib.mkIf publicClient secrets.hindsight-cloudflare-access.path;
}
