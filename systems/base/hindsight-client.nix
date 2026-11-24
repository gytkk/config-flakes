{
  config,
  lib,
  osConfig ? null,
  ...
}:
{
  age.secrets = lib.mkIf (osConfig == null) {
    hindsight-client-token = {
      file = ../../secrets/hindsight-client-token.age;
      mode = "0400";
    };
  };
  modules.pi.hindsightTokenFile =
    (if osConfig == null then config else osConfig).age.secrets.hindsight-client-token.path;
}
