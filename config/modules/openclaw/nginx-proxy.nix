{ config, lib, ... }:
let
  cfg = config.modules.openclaw;
  nginxAuthPath = "/etc/openclaw/nginx-gateway-auth.conf";

  proxyLocations = {
    "= /v1/models" = {
      return = ''200 '{"object":"list","data":[{"id":"openclaw/main","object":"model","created":0,"owned_by":"openclaw"},{"id":"openclaw/pro","object":"model","created":0,"owned_by":"openclaw"}]}' '';
      extraConfig = ''
        default_type application/json;
        add_header Content-Type application/json always;
      '';
    };
    "/" = {
      proxyPass = "http://127.0.0.1:${toString cfg.gatewayPort}";
      proxyWebsockets = true;
      extraConfig = ''
        proxy_read_timeout 10m;
        proxy_set_header Host $host:$server_port;
        proxy_set_header X-Forwarded-Host $host:$server_port;
        include ${nginxAuthPath};
      '';
    };
  };
in
{
  config = lib.mkIf cfg.enable {
    services.nginx = {
      enable = true;
      recommendedProxySettings = true;

      virtualHosts."openclaw-lan" = {
        serverName = "_";
        listen = [
          {
            addr = "0.0.0.0";
            port = cfg.lanProxyPort;
          }
        ];
        locations = proxyLocations;
      };

      virtualHosts."openclaw-public-origin" = {
        serverName = "_";
        listen = [
          {
            addr = "127.0.0.1";
            port = cfg.publicProxyPort;
          }
        ];
        locations = proxyLocations;
      };
    };

    networking.firewall.interfaces.${cfg.lanInterface}.allowedTCPPorts = [ cfg.lanProxyPort ];
  };
}
