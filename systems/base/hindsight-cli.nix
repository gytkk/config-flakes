{
  config,
  lib,
  pkgs,
  isWSL ? false,
  ...
}:
let
  endpoint = config.modules.pi.hindsightEndpoint;
  lanHosts = pkgs.writeText "hindsight-lan-hosts" "127.0.0.1 localhost\n192.168.0.2 hindsight.pylv.dev\n";
  tokenPath = lib.concatMapStringsSep ''"''${XDG_RUNTIME_DIR}"'' lib.escapeShellArg (
    lib.splitString "\${XDG_RUNTIME_DIR}" config.modules.pi.hindsightTokenFile
  );
in
{
  home.packages = [
    (pkgs.writeShellApplication {
      name = "hindsight";
      text = ''
        export HINDSIGHT_API_URL=${lib.escapeShellArg endpoint}
        HINDSIGHT_API_KEY=$(< ${tokenPath})
        if [[ ! "$HINDSIGHT_API_KEY" =~ ^[0-9a-f]{64}$ ]]; then
          echo "hindsight: invalid agenix client token" >&2
          exit 1
        fi
        export HINDSIGHT_API_KEY
        export NO_PROXY='*' no_proxy='*'
        exec ${lib.optionalString isWSL "${pkgs.bubblewrap}/bin/bwrap --dev-bind / / --ro-bind ${lanHosts} /etc/hosts --"} ${lib.getExe pkgs.hindsight-cli} "$@"
      '';
    })
  ];
}
