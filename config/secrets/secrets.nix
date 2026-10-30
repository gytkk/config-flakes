# 공개 키를 추가한 뒤 `agx -e secret-name.age`로 암호화하고 NixOS의 age.secrets에서 참조한다.
# NixOS는 /run/agenix를, Home Manager는 config.age.secrets.<name>.path를 사용한다.
let
  # Primary administrator SSH public key (for editing and recovery)
  agenixAdmin = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIJhE4Uakcz7usa0aetMqb99LYybOQ0I+sWKOiAidmBio";

  # Host SSH public keys (for host-specific secrets)
  pylv-sepia = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIC6EAZczgXONlXiwh946SidpRKSMw7fehg0u2L5SkHmd";
  pylv-onyx = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIG8Iug3LblOeh1VqwAwzynFPo5PGkPBsmZBHTYIZxCsy";

  # Devsisters machine SSH public keys
  devsisters-macbook = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIDdeZoty0uKpSDJ8sUGFwsMEEBYcuajo30lHlUHh8RMi";
  devsisters-macstudio = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHGIGT6Zgg4TW74umgyjlpk1b056LXDoC1kbBfPcqRuz";

  # Key groups
  adminRecipients = [ agenixAdmin ];
  allHosts = [
    pylv-sepia
    pylv-onyx
  ];
  devsistersHosts = [
    devsisters-macbook
    devsisters-macstudio
  ];
  deployMachineRecipients = adminRecipients ++ [ devsisters-macbook ];
  sepiaRecipients = adminRecipients ++ [ pylv-sepia ];
  onyxRecipients = adminRecipients ++ [ pylv-onyx ];
  devsistersRecipients = adminRecipients ++ devsistersHosts;
  allEnvironmentRecipients = adminRecipients ++ allHosts ++ devsistersHosts;
in
{
  # Deployment key backups (administrator/deployment Mac only)
  "astro-blog-deploy-key.age".publicKeys = deployMachineRecipients;
  "menu-deploy-key.age".publicKeys = deployMachineRecipients;
  "menu-r2-secret-access-key.age".publicKeys = deployMachineRecipients;

  # Menu CMS credential backup (workstations only; no servers)
  "menu-github-pat.age".publicKeys = adminRecipients ++ devsistersHosts;

  # Cloudflare administration token (administrator workstation only)
  "cloudflare-access-api-token.age".publicKeys = adminRecipients;

  # Cloudflare Tunnel token for pylv-sepia
  "cloudflare-tunnel-sepia-token.age".publicKeys = sepiaRecipients;

  # Discord bot token for openclaw
  "discord-bot-token.age".publicKeys = onyxRecipients;

  # Databricks OTEL token (devsisters environments only)
  "databricks-token.age".publicKeys = devsistersRecipients;

  # OpenAI API key for Neovim Minuet and Decisions clients
  "openai-api-key.age".publicKeys = allEnvironmentRecipients;
}
