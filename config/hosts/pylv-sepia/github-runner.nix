{ config, ... }:
{
  age.secrets.github-runner-ws-token.file = ../../secrets/github-runner-ws-token.age;

  services.github-runners.ws = {
    enable = true;
    url = "https://github.com/gytkk/ws";
    tokenFile = config.age.secrets.github-runner-ws-token.path;
    extraLabels = [ "pylv-sepia" ];
  };
}
