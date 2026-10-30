{ pkgs, ... }:
let
  root = "/srv/astro-blog";
  deployUser = "astro-blog-deploy";
  deployGroup = "astro-blog-deploy";
  deployScript = pkgs.writeText "astro-blog-deploy.py" (builtins.readFile ./astro-blog-deploy.py);
  deployCommand = pkgs.writeShellScript "astro-blog-deploy" ''
    exec ${pkgs.python3}/bin/python3 ${deployScript} --root ${root}
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
      ''restrict,command="${deployCommand}",no-pty,no-agent-forwarding,no-port-forwarding,no-X11-forwarding,no-user-rc ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIJ/J/YaTkH8JdmW576GXjNHHqMy7GkG4nWI6aTdw8gn2 astro-blog-deploy@pylv-sepia''
    ];
  };

  systemd.tmpfiles.rules = [
    "d ${root} 0775 root ${deployGroup} -"
    "d ${root}/releases 0755 ${deployUser} ${deployGroup} -"
  ];
}
