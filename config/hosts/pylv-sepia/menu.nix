{ config, pkgs, ... }:
let
  root = "/srv/menu";
  releases = "${root}/releases";
  host = "menu.pylv.dev";
  originPort = 12369;
  deployUser = "menu-deploy";
  deployGroup = "menu-deploy";
  deployScript = pkgs.writeText "menu-deploy.py" (builtins.readFile ./menu-deploy.py);
  deployCommand = pkgs.writeShellScript "menu-deploy" ''
    exec ${pkgs.python3}/bin/python3 ${deployScript} --root ${root}
  '';
  initialRelease = pkgs.runCommand "menu-initial-release" { } ''
    mkdir -p "$out/admin"
    printf '%s\n' '<!doctype html><title>Menu unavailable</title>' > "$out/index.html"
    printf '%s\n' '<!doctype html><title>Not found</title>' > "$out/404.html"
    printf '%s\n' '<rss version="2.0"></rss>' > "$out/rss.xml"
    printf '%s\n' '<?xml version="1.0"?><sitemapindex></sitemapindex>' > "$out/sitemap-index.xml"
    printf '%s\n' '<!doctype html><title>Admin unavailable</title>' > "$out/admin/index.html"
    printf '%s\n' '# deployment pending' > "$out/admin/config.yml"
  '';
in
{
  users.groups.${deployGroup} = { };
  users.users.${deployUser} = {
    isSystemUser = true;
    group = deployGroup;
    home = "/var/empty";
    createHome = false;
    shell = pkgs.bashInteractive;
    hashedPassword = "!";
    openssh.authorizedKeys.keys = [
      ''restrict,command="${deployCommand}",no-pty,no-agent-forwarding,no-port-forwarding,no-X11-forwarding,no-user-rc ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAz6TfmpUGb4lugITGkOzvPwEPYWiys6mcsvyqcyXZgP menu-deploy@pylv-sepia''
      ''restrict,command="${deployCommand}",no-pty,no-agent-forwarding,no-port-forwarding,no-X11-forwarding,no-user-rc ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAICEJZBeYjp2ypZhNs64IQJ7Lu0FCmI+pyZZlWrY/Cv6H menu-deploy-github-actions''
    ];
  };

  systemd.tmpfiles.rules = [
    "d ${root} 0775 root ${deployGroup} -"
    "d ${releases} 0755 ${deployUser} ${deployGroup} -"
    "L ${root}/current - - - - ${initialRelease}"
  ];

  services.nginx = {
    enable = true;
    recommendedGzipSettings = true;
    virtualHosts."menu-origin" = {
      serverName = host;
      listen = [
        {
          addr = "127.0.0.1";
          port = originPort;
        }
      ];
      root = "${root}/current";
      extraConfig = ''
        index index.html;
        error_page 404 /404.html;
        add_header Cache-Control "no-cache" always;
        add_header X-Content-Type-Options "nosniff" always;
        add_header X-Frame-Options "DENY" always;
        add_header Referrer-Policy "strict-origin-when-cross-origin" always;
        add_header Permissions-Policy "geolocation=(), microphone=(), camera=()" always;
      '';
      locations."/_astro/".extraConfig = ''
        try_files $uri =404;
        expires 1y;
        add_header Cache-Control "public, max-age=31536000, immutable" always;
        add_header X-Content-Type-Options "nosniff" always;
        add_header X-Frame-Options "DENY" always;
        add_header Referrer-Policy "strict-origin-when-cross-origin" always;
        add_header Permissions-Policy "geolocation=(), microphone=(), camera=()" always;
      '';
      locations."= /admin".return = "308 https://${host}/admin/";
      locations."/admin/config.yml".extraConfig = ''
        try_files $uri =404;
      '';
      locations."/admin/".extraConfig = ''
        try_files $uri $uri/ =404;
      '';
      locations."/".extraConfig = ''
        try_files $uri $uri/ =404;
      '';
    };
  };
}
