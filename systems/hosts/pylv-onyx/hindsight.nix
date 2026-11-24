{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.onyxHindsight;
  host = "hindsight.internal.pylv.dev";
  port = 8443;
  backendPort = 18888;
  # Independent sockets let LAN clients resolve memory even while tailscaled is down.
  dns =
    name: address:
    let
      zone = pkgs.writeText "${name}.zone" ''
        $ORIGIN ${host}.
        @ 60 IN SOA ns hostmaster (1 60 60 3600 60)
        @ 60 IN NS ns
        ns 60 IN A ${address}
        @ 60 IN A ${address}
      '';
      corefile = pkgs.writeText "${name}-Corefile" ''
        ${host}:53 {
          bind ${address}
          file ${zone} ${host}
        }
      '';
    in
    {
      description = "Private Hindsight DNS on ${address}";
      wantedBy = [ "multi-user.target" ];
      wants = [ "network-online.target" ];
      after = [ "network-online.target" ];
      unitConfig.StartLimitIntervalSec = 0;
      serviceConfig = {
        ExecStart = "${pkgs.coredns}/bin/coredns -conf ${corefile}";
        DynamicUser = true;
        AmbientCapabilities = [ "CAP_NET_BIND_SERVICE" ];
        CapabilityBoundingSet = [ "CAP_NET_BIND_SERVICE" ];
        NoNewPrivileges = true;
        Restart = "on-failure";
        RestartSec = "10s";
      };
    };
in
{
  options.services.onyxHindsight.enable = lib.mkEnableOption "the private Hindsight pilot on Onyx";

  config = lib.mkIf cfg.enable {
    networking.hosts."192.168.0.10" = [ host ];

    age.secrets = {
      hindsight-cloudflare-token.file = ../../../secrets/hindsight-cloudflare-token.age;
      hindsight-api-env.file = ../../../secrets/hindsight-api-env.age;
    };
    virtualisation.oci-containers = {
      backend = "podman";
      containers.hindsight = {
        # Upstream standalone 1.2.3, including embedded PostgreSQL.
        image = "ghcr.io/vectorize-io/hindsight@sha256:601cfdd1bd3f6053864c6458ee15259f065a01a3f83090b56a8ae57db67f8f38";
        pull = "missing";
        ports = [ "127.0.0.1:${toString backendPort}:8888" ];
        volumes = [ "hindsight-data:/home/hindsight/.pg0" ];
        environment = {
          HINDSIGHT_ENABLE_CP = "false";
          HINDSIGHT_API_LLM_PROVIDER = "openai";
          HINDSIGHT_API_TENANT_EXTENSION = "hindsight_api.extensions.builtin.tenant:ApiKeyTenantExtension";
        };
        environmentFiles = [ config.age.secrets.hindsight-api-env.path ];
      };
    };

    security.acme = {
      acceptTerms = true;
      certs.${host} = {
        dnsProvider = "cloudflare";
        # The private service zone does not serve the public ACME challenge TXT record.
        dnsResolver = "1.1.1.1:53";
        credentialFiles.CF_DNS_API_TOKEN_FILE = config.age.secrets.hindsight-cloudflare-token.path;
        group = "nginx";
        reloadServices = [ "nginx.service" ];
      };
    };
    services.nginx = {
      enable = true;
      virtualHosts.${host} = {
        useACMEHost = host;
        onlySSL = true;
        listen = [
          {
            addr = "0.0.0.0";
            inherit port;
            ssl = true;
          }
        ];
        locations."/v1/" = {
          proxyPass = "http://127.0.0.1:${toString backendPort}";
          extraConfig = ''
            allow 192.168.0.0/24;
            allow 100.64.0.0/10;
            deny all;
            proxy_read_timeout 360s;
          '';
        };
        locations."/".return = "404";
      };
    };

    systemd.services.hindsight-dns-lan = dns "hindsight-lan" "192.168.0.10";
    systemd.services.hindsight-dns-tailnet = dns "hindsight-tailnet" "100.84.249.30";
    networking.firewall.interfaces = lib.genAttrs [ "wlo1" "tailscale0" ] (_: {
      allowedTCPPorts = [
        53
        port
      ];
      allowedUDPPorts = [ 53 ];
    });
  };
}
