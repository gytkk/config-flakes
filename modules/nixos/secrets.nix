{
  username,
  config,
  lib,
  ...
}:
{
  age.secrets.hindsight-cloudflare-access = lib.mkIf (config.networking.hostName == "pylv-onyx") {
    file = ../../secrets/hindsight-cloudflare-access.age;
    owner = username;
    mode = "0400";
  };
  age.secrets.hindsight-client-token = {
    file = ../../secrets/hindsight-client-token.age;
    owner = username;
    group = "users";
    mode = "0400";
  };
  age.secrets."openai-api-key" = {
    file = ../../secrets/openai-api-key.age;
    owner = username;
    group = "users";
    mode = "0400";
  };
}
