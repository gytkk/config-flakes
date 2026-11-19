{
  config,
  lib,
  username,
  homeDirectory,
  ...
}:

{
  # Import base configuration
  imports = [
    ../default.nix
    ./tools.nix
  ];

  modules.pi.settingsFile = ../../../modules/pi/files/settings-devsisters.json;

  # The current shadow rollout records personal sessions only.
  modules.agentSessionRecord.enable = lib.mkForce false;

  # Devsisters 특화 환경 변수
  home.sessionVariables = {
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
    file = ../../../secrets/databricks-token.age;
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
}
