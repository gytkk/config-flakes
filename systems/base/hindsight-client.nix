{
  config,
  lib,
  osConfig ? null,
  flakeDirectory,
  isWSL ? false,
  ...
}:
let
  localServer = osConfig != null && osConfig.networking.hostName == "pylv-sepia";
  secrets = (if osConfig == null then config else osConfig).age.secrets;
in
{
  imports = [ ./hindsight-cli.nix ];
  age.secrets = lib.mkIf (osConfig == null) {
    hindsight-client-token = {
      file = ../../secrets/hindsight-client-token.age;
      mode = "0400";
    };
  };
  home.file.".hindsight/coding-agent.json" = lib.mkIf config.modules.pi.enable {
    source = config.lib.file.mkOutOfStoreSymlink "${flakeDirectory}/modules/pi/files/hindsight-client.json";
  };
  modules.pi.hindsightTokenFile = secrets.hindsight-client-token.path;
  modules.pi.hindsightLan = isWSL;
  modules.pi.hindsightEndpoint =
    if localServer then
      "http://127.0.0.1:8888"
    else if isWSL then
      "https://hindsight.pylv.dev"
    else
      "https://pylv-sepia.tailbbb9bf.ts.net";
}
