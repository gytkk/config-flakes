{ pkgs, username, ... }@args:
let
  sharedAuthorizedKeys = [
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIJhE4Uakcz7usa0aetMqb99LYybOQ0I+sWKOiAidmBio gytk.kim@gmail.com"
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIDdeZoty0uKpSDJ8sUGFwsMEEBYcuajo30lHlUHh8RMi devsisters-macbook"
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHGIGT6Zgg4TW74umgyjlpk1b056LXDoC1kbBfPcqRuz devsisters-macstudio"
  ];
in
{
  security.sudo = {
    enable = true;
    wheelNeedsPassword = false;
  };

  users.users.root.openssh.authorizedKeys.keys =
    sharedAuthorizedKeys ++ (args.extraPublicKeys or [ ]);

  users.users.${username} = {
    isNormalUser = true;
    extraGroups = [ "wheel" ];
    shell = pkgs.zsh;
    openssh.authorizedKeys.keys = sharedAuthorizedKeys;
  };
}
