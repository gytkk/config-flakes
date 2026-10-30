{
  config,
  lib,
  pkgs,
  inputs,
  username,
  homeDirectory,
  ...
}:

{
  # Import base configuration
  imports = [
    ../default.nix
    ../../modules/terraform
  ];

  # The current shadow rollout records personal sessions only.
  modules.agentSessionRecord.enable = lib.mkForce false;

  # Devsisters 특화 패키지 추가
  home.packages = with pkgs; [
    # Scala (default to 2.12, compatible with existing projects)
    scala_2_12

    # Authentication
    saml2aws
    vault
    inputs.keycloak2aws.packages.${pkgs.stdenv.hostPlatform.system}.kc2aws
    inputs.devsisters-wg.packages.${pkgs.stdenv.hostPlatform.system}.wg-cli

    # Ruby
    pkgs.stable-25_05.ruby_3_2

    # Databricks CLI (Go 구현체, GitHub 릴리스 바이너리)
    databricks-cli

    # Pantry CLI (teamdev.ai 배포 도구, main 빌드 바이너리, 수동 업데이트)
    pantry

    # Custom scripts
    (pkgs.writeShellScriptBin "sign" (builtins.readFile ./scripts/sign))
    (pkgs.writeShellScriptBin "dlg" (builtins.readFile ./scripts/login))
  ];

  # Terraform 모듈 설정
  modules.terraform = {
    enable = true;
    runEnv = {
      AWS_PROFILE = "saml";
    };
  };

  # Devsisters 특화 Git 설정
  programs.git.settings.user = {
    email = lib.mkForce "gyutak.kim@devsisters.com";
    name = lib.mkForce "Gyutak Kim";
  };

  # Devsisters 특화 환경 변수
  home.sessionVariables = {
    VAULT_ADDR = "https://vault.devsisters.cloud";
    # SBT Java 호환성 설정
    SBT_OPTS = "-Xmx2G -XX:+UseG1GC";
    # 일부 OTLP 클라이언트는 공통 프로토콜만 읽으므로 Databricks가 지원하는 HTTP/protobuf를 명시한다.
    OTEL_EXPORTER_OTLP_PROTOCOL = "http/protobuf";
    OTEL_METRICS_EXPORTER = "otlp";
    OTEL_EXPORTER_OTLP_METRICS_PROTOCOL = "http/protobuf";
    OTEL_EXPORTER_OTLP_METRICS_ENDPOINT = "https://devsisters-seusw2.cloud.databricks.com/api/2.0/otel/v1/metrics";
    OTEL_METRIC_EXPORT_INTERVAL = "10000";
    OTEL_LOGS_EXPORTER = "otlp";
    OTEL_EXPORTER_OTLP_LOGS_PROTOCOL = "http/protobuf";
    OTEL_EXPORTER_OTLP_LOGS_ENDPOINT = "https://devsisters-seusw2.cloud.databricks.com/api/2.0/otel/v1/logs";
    OTEL_LOGS_EXPORT_INTERVAL = "5000";
    OTEL_LOG_USER_PROMPTS = "1";
  };

  # Databricks OTEL token for the seusw2 telemetry workspace (decrypted by agenix).
  # Same destination as the org's Claude Code remote managed settings.
  age.secrets.databricks-token = {
    file = ../../secrets/databricks-token.age;
  };

  # Claude Code reads the token through otelHeadersHelper, so sessions started
  # outside an interactive shell still authenticate.
  modules.claude.otelTokenFile = config.age.secrets.databricks-token.path;
  modules.claude.mumu.enable = true;

  # Construct OTEL headers at runtime using the decrypted token (shells and Codex)
  programs.zsh.initContent = lib.mkAfter ''
    if [ -f "${config.age.secrets.databricks-token.path}" ]; then
      _dbx_token=$(cat "${config.age.secrets.databricks-token.path}")
      export OTEL_EXPORTER_OTLP_METRICS_HEADERS="content-type=application/x-protobuf,Authorization=Bearer $_dbx_token,X-Databricks-UC-Table-Name=telemetry.ai_observability.agent_otel_metrics"
      export OTEL_EXPORTER_OTLP_LOGS_HEADERS="content-type=application/x-protobuf,Authorization=Bearer $_dbx_token,X-Databricks-UC-Table-Name=telemetry.ai_observability.agent_otel_logs"
      unset _dbx_token
    fi
  '';

  # Add gem binaries to PATH
  home.sessionPath = [
    "$HOME/.gem/ruby/3.2.0/bin"
  ];
}
