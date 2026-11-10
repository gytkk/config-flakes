{
  inputs,
  modulesPath,
  pkgs,
  ...
}:
{
  imports = [
    (modulesPath + "/installer/scan/not-detected.nix")
    (modulesPath + "/profiles/qemu-guest.nix")
    inputs.disko.nixosModules.disko
    inputs.copyparty.nixosModules.default
    ./disk-config.nix
    ./ghost.nix
    ./trading-agent.nix
    ./astro-blog-legacy.nix
    ./menu.nix
    ./github-runner.nix
    ../../../modules/nixos
  ];

  time.timeZone = "Asia/Seoul";
  networking.hostName = "pylv-sepia";

  # Concurrent package builds can exhaust this server's memory.
  nix.settings.max-jobs = 1;

  boot.loader.grub = {
    efiSupport = true;
    efiInstallAsRemovable = true;
  };

  # Copyparty - file server
  services.copyparty = {
    enable = true;
    settings = {
      i = "0.0.0.0";
      p = [ 3923 ];
    };
    volumes = {
      "/" = {
        path = "/srv/copyparty";
        access = {
          r = "*"; # 모든 사용자 읽기 허용
          w = "*"; # 모든 사용자 쓰기 허용
        };
        flags = {
          e2d = true; # enable file indexing
        };
      };
    };
  };

  # Copyparty 데이터 디렉토리 생성
  systemd.tmpfiles.rules = [
    "d /srv/copyparty 0755 copyparty copyparty -"
  ];

  # Immich - self-hosted photo/video management
  services.immich = {
    enable = true;
    host = "0.0.0.0";
    port = 2283;
  };

  # Cloudflare Tunnel (token-based, managed via Zero Trust dashboard)
  age.secrets.cloudflare-tunnel-sepia-token = {
    file = ../../../secrets/cloudflare-tunnel-sepia-token.age;
    owner = "cloudflared";
    group = "cloudflared";
  };

  systemd.services.cloudflared-tunnel = {
    description = "Cloudflare Tunnel";
    after = [ "network-online.target" ];
    wants = [ "network-online.target" ];
    wantedBy = [ "multi-user.target" ];
    script = ''
      ${pkgs.cloudflared}/bin/cloudflared tunnel --no-autoupdate run \
        --token "$(cat /run/agenix/cloudflare-tunnel-sepia-token)"
    '';
    serviceConfig = {
      Restart = "always";
      RestartSec = 5;
      User = "cloudflared";
      Group = "cloudflared";
    };
  };

  users.users.cloudflared = {
    isSystemUser = true;
    group = "cloudflared";
  };
  users.groups.cloudflared = { };

  # Host-specific packages
  environment.systemPackages = with pkgs; [
    cloudflared
    # Kubernetes tools
    kubectl
    k9s
  ];

  # Firewall - open ports for services
  networking.firewall.allowedTCPPorts = [
    22 # OpenSSH
    3923 # Copyparty
    2283 # Immich
  ];

  system.stateVersion = "25.11";
}
