{
  config,
  lib,
  pkgs,
  ...
}:
let
  wheels = pkgs.linkFarm "hindsight-wheels" (
    lib.mapAttrsToList (url: sha256: {
      name = builtins.baseNameOf url;
      path = pkgs.fetchurl { inherit url sha256; };
    }) (lib.importJSON ./hindsight-wheels.json)
  );
  hindsight = pkgs.runCommand "hindsight-api-1.2.3" { nativeBuildInputs = [ pkgs.makeWrapper ]; } ''
    export HOME=$TMPDIR UV_OFFLINE=1 UV_NO_CACHE=1 UV_LINK_MODE=copy
    ${pkgs.uv}/bin/uv venv --python ${pkgs.python312}/bin/python3 $out/env
    ${pkgs.uv}/bin/uv pip install --python $out/env/bin/python --no-index --find-links ${wheels} 'hindsight-api-slim[local-onnx]==1.2.3'
    export LD_LIBRARY_PATH=${
      lib.makeLibraryPath [
        pkgs.stdenv.cc.cc.lib
        pkgs.zlib
      ]
    }
    $out/env/bin/python -c 'import hindsight_api.main, onnxruntime; from transformers import AutoTokenizer'
    mkdir -p $out/bin
    for program in hindsight-api hindsight-admin; do
      makeWrapper $out/env/bin/$program $out/bin/$program --prefix LD_LIBRARY_PATH : "$LD_LIBRARY_PATH"
    done
    $out/bin/hindsight-admin chatgpt login --help >/dev/null
  '';
  model = pkgs.linkFarm "multilingual-e5-small-onnx" (
    lib.mapAttrsToList
      (name: sha256: {
        inherit name;
        path = pkgs.fetchurl {
          inherit sha256;
          url = "https://huggingface.co/intfloat/multilingual-e5-small/resolve/614241f622f53c4eeff9890bdc4f31cfecc418b3/onnx/${name}";
        };
      })
      {
        "model.onnx" = "ca456c06b3a9505ddfd9131408916dd79290368331e7d76bb621f1cba6bc8665";
        "config.json" = "bbb7c1333fc4b3e27fbc9cd5d2070aabcc1d4dfb99917c3633e772f97545a6b6";
        "tokenizer.json" = "0b44a9d7b51c3c62626640cda0e2c2f70fdacdc25bbbd68038369d14ebdf4c39";
        "tokenizer_config.json" = "a1d6bc8734a6f635dc158508bef000f8e2e5a759c7d92f984b2c86e5ff53425b";
        "special_tokens_map.json" = "d05497f1da52c5e09554c0cd874037a083e1dc1b9cfd48034d1c717f1afc07a7";
        "sentencepiece.bpe.model" = "cfc8146abe2a0488e9e2a0c56de7952f7c11ab059eca145a0a727afce0db2865";
      }
  );
in
{
  environment.systemPackages = [ hindsight ];
  users.groups.hindsight = { };
  users.users.hindsight = {
    isSystemUser = true;
    group = "hindsight";
    home = "/var/lib/hindsight";
  };
  systemd.tmpfiles.rules = [ "d /var/lib/hindsight 0700 hindsight hindsight -" ];
  services.postgresql = {
    enable = true;
    ensureDatabases = [ "hindsight" ];
    ensureUsers = [
      {
        name = "hindsight";
        ensureDBOwnership = true;
      }
    ];
    extensions = ps: [ ps.pgvector ];
  };
  systemd.services.hindsight-db = {
    description = "Prepare the Hindsight vector extension";
    after = [ "postgresql.target" ];
    requires = [ "postgresql.target" ];
    serviceConfig = {
      User = "postgres";
      Type = "oneshot";
      RemainAfterExit = true;
      ExecStart = ''${config.services.postgresql.package}/bin/psql -d hindsight -v ON_ERROR_STOP=1 -c "CREATE EXTENSION IF NOT EXISTS vector"'';
    };
  };
  systemd.services.hindsight = {
    description = "Hindsight memory API";
    wantedBy = [ "multi-user.target" ];
    after = [
      "hindsight-db.service"
      "network-online.target"
    ];
    requires = [ "hindsight-db.service" ];
    wants = [ "network-online.target" ];
    unitConfig.ConditionPathExists = "/var/lib/hindsight/.hindsight/chatgpt/credentials.json";
    environment = {
      HOME = "/var/lib/hindsight";
      HINDSIGHT_API_HOST = "127.0.0.1";
      HINDSIGHT_API_PORT = "8888";
      HINDSIGHT_API_DATABASE_URL = "postgresql://hindsight@/hindsight?host=/run/postgresql";
      HINDSIGHT_API_LLM_PROVIDER = "chatgpt";
      HINDSIGHT_API_LLM_MODEL = "gpt-6-luna";
      HINDSIGHT_API_EMBEDDINGS_PROVIDER = "onnx";
      HINDSIGHT_API_EMBEDDINGS_ONNX_MODEL_PATH = "${model}/model.onnx";
      HINDSIGHT_API_EMBEDDINGS_ONNX_TOKENIZER_NAME_OR_PATH = "${model}";
      HINDSIGHT_API_EMBEDDINGS_ONNX_DIMENSIONS = "384";
      HINDSIGHT_API_RERANKER_PROVIDER = "rrf";
      HINDSIGHT_API_TENANT_EXTENSION = "hindsight_api.extensions.builtin.tenant:ApiKeyTenantExtension";
    };
    script = ''
      if [ ! -e client-key ]; then
        ${pkgs.openssl}/bin/openssl rand -hex 32 > client-key.new
        mv client-key.new client-key
      fi
      export HINDSIGHT_API_TENANT_API_KEY="$(<client-key)"
      exec ${hindsight}/bin/hindsight-api
    '';
    serviceConfig = {
      User = "hindsight";
      Group = "hindsight";
      WorkingDirectory = "/var/lib/hindsight";
      UMask = "0077";
      Restart = "on-failure";
      RestartSec = "10s";
      PrivateTmp = true;
      NoNewPrivileges = true;
      ProtectSystem = "strict";
      ReadWritePaths = [ "/var/lib/hindsight" ];
    };
  };
}
