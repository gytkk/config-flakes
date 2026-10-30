{
  config,
  lib,
  pkgs,
  homeDirectory,
  ...
}:
let
  cfg = config.modules.openclaw;
  stateDir = toString cfg.stateDir;
  configPath = "${stateDir}/openclaw.json";
  gatewayTokenPath = "${stateDir}/gateway-auth-token";
  nginxAuthPath = "/etc/openclaw/nginx-gateway-auth.conf";
in
{
  imports = [ ./nginx-proxy.nix ];

  options.modules.openclaw = {
    enable = lib.mkEnableOption "host integration for a user-managed OpenClaw installation";

    gatewayPort = lib.mkOption {
      type = lib.types.port;
      default = 18789;
      description = "Loopback OpenClaw gateway port.";
    };

    lanProxyPort = lib.mkOption {
      type = lib.types.port;
      default = 18790;
      description = "LAN nginx proxy port for OpenClaw.";
    };

    publicProxyPort = lib.mkOption {
      type = lib.types.port;
      default = 18791;
      description = "Loopback public-origin nginx proxy port for OpenClaw.";
    };

    lanInterface = lib.mkOption {
      type = lib.types.str;
      default = "";
      description = "Network interface that receives LAN OpenClaw traffic.";
    };

    stateDir = lib.mkOption {
      type = lib.types.str;
      default = "${homeDirectory}/.openclaw";
      description = "Mutable OpenClaw state and installation directory.";
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.lanInterface != "";
        message = "modules.openclaw.lanInterface must be set when modules.openclaw.enable is true.";
      }
      {
        assertion = lib.hasPrefix "/" stateDir;
        message = "modules.openclaw.stateDir must be an absolute path.";
      }
      {
        assertion = !(lib.hasPrefix "/nix/store/" stateDir);
        message = "modules.openclaw.stateDir must point to mutable host storage, not the Nix store.";
      }
    ];

    environment.systemPackages = with pkgs; [
      chromium
      libcap
      openssl
    ];

    environment.sessionVariables = {
      OPENCLAW_CONFIG_PATH = configPath;
      OPENCLAW_STATE_DIR = stateDir;
    };

    system.activationScripts.openclawSyncNginxAuth = lib.stringAfter [ "etc" ] ''
      if [ -L /etc/openclaw ]; then
        ${pkgs.coreutils}/bin/rm /etc/openclaw
      fi
      ${pkgs.coreutils}/bin/mkdir -p /etc/openclaw

      gateway_token=
      if [ -f ${lib.escapeShellArg configPath} ]; then
        gateway_token="$(${pkgs.jq}/bin/jq -er '.gateway.auth.token | select(type == "string") // empty' ${lib.escapeShellArg configPath} 2>/dev/null || true)"
      fi
      if [ -z "$gateway_token" ] && [ -s ${lib.escapeShellArg gatewayTokenPath} ]; then
        gateway_token="$(${pkgs.coreutils}/bin/cat ${lib.escapeShellArg gatewayTokenPath})"
      fi

      tmp_auth="$(${pkgs.coreutils}/bin/mktemp)"
      if [ -n "$gateway_token" ]; then
        printf 'proxy_set_header Authorization "Bearer %s";\n' "$gateway_token" > "$tmp_auth"
      else
        printf 'proxy_set_header Authorization "";\n' > "$tmp_auth"
        echo "warning: OpenClaw gateway token is unavailable; proxied requests will not authenticate" >&2
      fi
      ${pkgs.coreutils}/bin/install -m 440 -o root -g ${config.services.nginx.group} "$tmp_auth" ${nginxAuthPath}
      ${pkgs.coreutils}/bin/rm -f "$tmp_auth"
    '';
  };
}
