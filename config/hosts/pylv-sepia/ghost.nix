{ lib, pkgs, ... }:
let
  ghostContentDir = "/var/lib/ghost";
  ghostBackupDir = "/var/lib/ghost-backups";
  ghostPort = 2368;
  ghostOriginPort = 12368;
  ghostFqdn = "ghost.pylv.dev";
in
{
  systemd.tmpfiles.rules = [
    "d ${ghostContentDir} 0750 1000 1000 -"
    "d ${ghostBackupDir} 0700 1000 1000 -"
  ];

  virtualisation.oci-containers = {
    backend = "podman";
    containers.ghost = {
      # Ghost 6.53.0-bookworm
      image = "docker.io/library/ghost@sha256:94b71e5058d8d0adbb76267e007da09d049f00fe285a186fac2c5a5641e256e8";
      pull = "missing";
      user = "1000:1000";
      volumes = [
        "${ghostContentDir}:/var/lib/ghost/content"
      ];
      environment = {
        NODE_ENV = "production";
        url = "https://${ghostFqdn}";
        database__client = "sqlite3";
        database__connection__filename = "/var/lib/ghost/content/data/ghost.db";
        server__host = "127.0.0.1";
        server__port = "2368";
        logging__transports = ''["stdout"]'';
      };
      capabilities = {
        ALL = false;
      };
      extraOptions = [
        "--network=host"
        "--read-only"
        "--memory=2g"
        "--cpus=2"
        "--pids-limit=256"
        "--tmpfs=/tmp:rw,nosuid,nodev,noexec,size=64m"
        "--security-opt=no-new-privileges"
        ''--health-cmd=node -e "require('http').get('http://127.0.0.1:2368/',r=>process.exit(r.statusCode>=200&&r.statusCode<400?0:1)).on('error',()=>process.exit(1))"''
        "--health-interval=30s"
        "--health-start-period=60s"
        "--health-timeout=5s"
        "--health-retries=3"
        "--health-on-failure=kill"
      ];
      podman.sdnotify = "healthy";
    };
  };

  systemd.services.podman-ghost = {
    unitConfig = {
      StartLimitIntervalSec = 300;
      StartLimitBurst = 5;
    };
    serviceConfig = {
      Restart = "on-failure";
      RestartSec = "10s";
      TimeoutStartSec = lib.mkForce "3min";
      UMask = "0027";
      PrivateTmp = true;
      ProtectClock = true;
      ProtectHome = true;
      ProtectKernelLogs = true;
      ProtectKernelModules = true;
      LockPersonality = true;
      RestrictRealtime = true;
    };
  };

  services.nginx = {
    enable = true;
    virtualHosts."ghost-origin" = {
      serverName = ghostFqdn;
      listen = [
        {
          addr = "127.0.0.1";
          port = ghostOriginPort;
        }
      ];
      extraConfig = ''
        client_max_body_size 50m;
      '';
      locations."/".proxyPass = "http://127.0.0.1:${toString ghostPort}";
      locations."/".extraConfig = ''
        proxy_http_version 1.1;
        proxy_set_header Host ${ghostFqdn};
        proxy_set_header X-Forwarded-Host ${ghostFqdn};
        proxy_set_header X-Forwarded-Proto https;
        proxy_set_header X-Forwarded-Port 443;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Real-IP $remote_addr;
      '';
    };
  };

  systemd.timers.ghost-backup = {
    description = "Daily local Ghost backup";
    wantedBy = [ "timers.target" ];
    timerConfig = {
      OnCalendar = "daily";
      Persistent = true;
      RandomizedDelaySec = "1h";
      Unit = "ghost-backup.service";
    };
  };

  systemd.services.ghost-backup = {
    description = "Create a coherent local Ghost backup";
    after = [ "podman-ghost.service" ];
    conflicts = [ "podman-ghost.service" ];
    onFailure = [ "podman-ghost.service" ];
    onSuccess = [ "podman-ghost.service" ];
    path = with pkgs; [
      coreutils
      findutils
      gzip
      gnutar
      sqlite
    ];
    serviceConfig = {
      Type = "oneshot";
      User = "1000";
      Group = "1000";
      UMask = "0077";
      Nice = 10;
      IOSchedulingClass = "best-effort";
      IOSchedulingPriority = 7;
      NoNewPrivileges = true;
      PrivateDevices = true;
      PrivateTmp = true;
      ProtectClock = true;
      ProtectControlGroups = true;
      ProtectHome = true;
      ProtectKernelLogs = true;
      ProtectKernelModules = true;
      ProtectKernelTunables = true;
      ProtectSystem = "strict";
      ReadOnlyPaths = [ ghostContentDir ];
      ReadWritePaths = [ ghostBackupDir ];
      CapabilityBoundingSet = "";
      LockPersonality = true;
      MemoryDenyWriteExecute = true;
      RestrictAddressFamilies = [ "AF_UNIX" ];
      RestrictRealtime = true;
    };
    script = ''
      database="${ghostContentDir}/data/ghost.db"
      if [ ! -f "$database" ]; then
        echo "Ghost database not found at $database" >&2
        exit 1
      fi

      timestamp="$(date --utc +%Y%m%dT%H%M%SZ)"
      staging="$(mktemp --directory --tmpdir=${ghostBackupDir} .ghost-backup.XXXXXX)"
      partial="${ghostBackupDir}/ghost-$timestamp.tar.gz.partial"
      final="${ghostBackupDir}/ghost-$timestamp.tar.gz"
      trap 'rm -rf "$staging" "$partial"' EXIT

      install -d -m 0700 "$staging/data"
      sqlite3 -readonly "$database" ".timeout 10000" ".backup '$staging/data/ghost.db'"
      sqlite3 -readonly "$staging/data/ghost.db" "PRAGMA integrity_check;" | \
        { read -r result; [ "$result" = "ok" ]; }

      tar \
        --create \
        --gzip \
        --file "$partial" \
        --directory "${ghostContentDir}" \
        --exclude "./data/ghost.db" \
        --exclude "./data/ghost.db-journal" \
        --exclude "./data/ghost.db-shm" \
        --exclude "./data/ghost.db-wal" \
        . \
        --directory "$staging" \
        data/ghost.db

      mv "$partial" "$final"
      find "${ghostBackupDir}" \
        -maxdepth 1 \
        -type f \
        -name 'ghost-*.tar.gz' \
        -mtime +29 \
        -delete
    '';
  };
}
