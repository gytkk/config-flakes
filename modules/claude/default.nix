{
  config,
  pkgs,
  lib,
  flakeDirectory,
  ...
}:

let
  cfg = config.modules.claude;
  agentCoreOutput = import ../../agent-core/nix/render.nix { inherit pkgs; } { runtime = "claude"; };
  herdrSubagentsSource = lib.fileset.toSource {
    root = ../..;
    fileset = lib.fileset.unions [
      ./files/herdr-subagents
      ../herdr/files/jsonl-tail.ts
    ];
  };
  claude = "${pkgs.claude-code}/bin/claude";
  timeout = "${pkgs.coreutils}/bin/timeout --foreground";
  mkSymlink = path: config.lib.file.mkOutOfStoreSymlink "${flakeDirectory}/modules/claude/${path}";
  marketplaces = [
    "anthropics/skills"
    "anthropics/claude-code"
    "anthropics/claude-plugins-official"
    "backnotprop/plannotator"
    "openai/codex-plugin-cc"
  ]
  ++ lib.optional cfg.mumu.enable "devsisters/mumu";

  # Local marketplaces sourced from the flake working tree.
  # Each entry is registered via `claude plugin marketplace add <path>`.
  localMarketplaces = [
    {
      name = "gytkk";
      path = "${flakeDirectory}/modules/claude/marketplace";
    }
  ];

  # plugin-name@marketplace-name
  plugins = [
    "document-skills@anthropic-agent-skills"
    "commit-commands@claude-code-plugins"
    "security-guidance@claude-code-plugins"

    "ralph-loop@claude-plugins-official"
    "superpowers@claude-plugins-official"

    "slack@claude-plugins-official"

    "gopls-lsp@claude-plugins-official"
    "rust-analyzer-lsp@claude-plugins-official"
    "typescript-lsp@claude-plugins-official"
    "pyright-lsp@claude-plugins-official"

    # backnotprop/plannotator — visual plan annotation and review
    "plannotator@plannotator"

    # openai/codex-plugin-cc — Official Codex plugin for Claude Code
    "codex@openai-codex"

    # local gytkk marketplace (modules/claude/marketplace) - runtime-specific skill and LSP plugins
    "devils-advocate@gytkk"
    "terraform-ls@gytkk"
    "nixd-lsp@gytkk"
  ]
  ++ lib.optional cfg.mumu.enable "mumu@mumu";

  mumuSettings = pkgs.writeText "claude-mumu-settings.json" (
    builtins.toJSON {
      enabledPlugins."mumu@mumu" = true;
      extraKnownMarketplaces.mumu = {
        source = {
          source = "github";
          repo = "devsisters/mumu";
        };
        autoUpdate = true;
      };
    }
  );
  claudePackage =
    if cfg.mumu.enable then
      pkgs.writeShellScriptBin "claude" ''
        exec ${claude} --settings ${mumuSettings} "$@"
      ''
    else
      pkgs.claude-code;

  mcpCommands = [
    {
      name = "context7";
      cmd = "mcp add -s user --transport http context7 https://mcp.context7.com/mcp";
    }
  ];
  removedMcpServers = [
    "cloudflare"
    "notion"
  ];

  mkClaudeCommand = duration: command: ''
    if ${timeout} ${duration} ${claude} ${command} < /dev/null >> "$SETUP_LOG" 2>&1; then
      log "  -> OK"
    else
      log "  -> FAILED (exit $?)"
    fi'';

  mkMarketplaceRegistration = mp: ''
    if ! printf '%s\n' "$MARKETPLACE_CACHE" | ${pkgs.ripgrep}/bin/rg -q --fixed-strings -- "${mp}"; then
      log "Adding marketplace: ${mp}"
      ${mkClaudeCommand "60s" "plugin marketplace add ${mp}"}
    else
      log "Marketplace already registered: ${mp}"
    fi'';

  mkLocalMarketplaceRegistration = mp: ''
    LOCAL_MP_NAME=${lib.escapeShellArg mp.name}
    LOCAL_MP_PATH=${lib.escapeShellArg mp.path}
    EXISTING_SOURCE=""
    EXISTING_PATH=""
    if [ -f "$KNOWN_MARKETPLACES" ]; then
      EXISTING_SOURCE=$(${pkgs.jq}/bin/jq -r --arg n "$LOCAL_MP_NAME" '.[$n].source.source // ""' "$KNOWN_MARKETPLACES" 2>/dev/null || echo "")
      EXISTING_PATH=$(${pkgs.jq}/bin/jq -r --arg n "$LOCAL_MP_NAME" '.[$n].source.path // ""' "$KNOWN_MARKETPLACES" 2>/dev/null || echo "")
    fi
    if [ "$EXISTING_SOURCE" = "github" ]; then
      log "Removing stale github marketplace: $LOCAL_MP_NAME"
      ${timeout} 30s ${claude} plugin marketplace remove "$LOCAL_MP_NAME" < /dev/null >> "$SETUP_LOG" 2>&1 || log "  -> remove FAILED (exit $?)"
      EXISTING_SOURCE=""
      EXISTING_PATH=""
    fi
    if [ "$EXISTING_PATH" != "$LOCAL_MP_PATH" ]; then
      log "Adding local marketplace: $LOCAL_MP_NAME -> $LOCAL_MP_PATH"
      ${mkClaudeCommand "60s" ''plugin marketplace add "$LOCAL_MP_PATH"''}
    else
      log "Local marketplace already registered: $LOCAL_MP_NAME"
    fi'';

  mkPluginSetup = plugin: ''
    HAS_USER_SCOPE=0
    if [ -f "$INSTALLED_PLUGINS" ]; then
      if [ "$(${pkgs.jq}/bin/jq -r --arg p "${plugin}" '
            (.plugins[$p] // []) | map(select(.scope == "user")) | length
          ' "$INSTALLED_PLUGINS" 2>/dev/null)" != "0" ]; then
        HAS_USER_SCOPE=1
      fi
    fi
    if [ "$HAS_USER_SCOPE" = "0" ]; then
      PLUGIN_ACTION="install"
      log "Installing plugin (user scope): ${plugin}"
    else
      PLUGIN_ACTION="update"
      log "Updating plugin (user scope): ${plugin}"
    fi
    ${mkClaudeCommand "60s" ''plugin "$PLUGIN_ACTION" -s user ${plugin}''}'';

  mkMcpRemoval = name: ''
    if printf '%s\n' "$MCP_CACHE" | ${pkgs.ripgrep}/bin/rg -q --fixed-strings -- "${name}"; then
      log "Removing MCP server: ${name}"
      if ${timeout} 30s ${claude} mcp remove -s user ${name} < /dev/null >> "$SETUP_LOG" 2>&1; then
        log "  -> OK"
        MCP_CACHE=$(${timeout} 15s ${claude} mcp list < /dev/null 2>/dev/null || echo "")
      else
        log "  -> FAILED (exit $?)"
      fi
    fi'';

  mkMcpRegistration =
    { name, cmd }:
    ''
      if ! printf '%s\n' "$MCP_CACHE" | ${pkgs.ripgrep}/bin/rg -q --fixed-strings -- "${name}"; then
        log "Adding MCP server: ${name}"
        ${mkClaudeCommand "30s" cmd}
      else
        log "MCP server already registered: ${name}"
      fi'';

  mkMarketplacesSetup =
    { marketplaces, localMarketplaces }:
    ''
      # Register marketplaces (failures are non-fatal)
      ${lib.concatMapStringsSep "\n    " mkMarketplaceRegistration marketplaces}

      # 같은 이름의 GitHub marketplace를 먼저 제거해
      # 로컬 marketplace 경로가 충돌 없이 우선하도록 한다.
      KNOWN_MARKETPLACES="$HOME/.claude/plugins/known_marketplaces.json"
      ${lib.concatMapStringsSep "\n    " mkLocalMarketplaceRegistration localMarketplaces}

      log "Updating marketplace index..."
      ${timeout} 30s ${claude} plugin marketplace update < /dev/null >> "$SETUP_LOG" 2>&1 || log "Marketplace update failed"
    '';

  mkPluginsSetup = plugins: ''
    INSTALLED_PLUGINS="$HOME/.claude/plugins/installed_plugins.json"

    # Temporarily run plugin install/update against a copy so activation
    # does not mutate the repository-backed settings symlink.
    SETTINGS_FILE="$HOME/.claude/settings.json"
    SETTINGS_LINK_TARGET=""
    if [ -L "$SETTINGS_FILE" ]; then
      SETTINGS_LINK_TARGET=$(${pkgs.coreutils}/bin/readlink -f "$SETTINGS_FILE")
      ${pkgs.coreutils}/bin/cp --remove-destination "$SETTINGS_LINK_TARGET" "$SETTINGS_FILE"
      ${pkgs.coreutils}/bin/chmod u+w "$SETTINGS_FILE"
      log "Temporarily made settings.json writable for plugin operations"
    fi

    # 다른 프로젝트에서 설치를 다시 묻지 않도록 플러그인은 항상 user scope에 설치한다.
    # installed_plugins.json에서는 scope를 직접 확인해 project 항목과 구분한다.
    ${lib.concatMapStringsSep "\n    " mkPluginSetup plugins}

    if [ -n "$SETTINGS_LINK_TARGET" ]; then
      ${pkgs.coreutils}/bin/ln -sf "$SETTINGS_LINK_TARGET" "$SETTINGS_FILE"
      log "Restored settings.json symlink"
    fi
  '';

  mkMcpSetup =
    { mcpCommands, removedMcpServers }:
    ''
      # Cache MCP server list once to avoid repeated calls
      MCP_CACHE=$(${timeout} 15s ${claude} mcp list < /dev/null 2>/dev/null || echo "")

      ${lib.concatMapStringsSep "\n    " mkMcpRemoval removedMcpServers}
      ${lib.concatMapStringsSep "\n    " mkMcpRegistration mcpCommands}
    '';

  mkClaudeCodeSetup =
    {
      marketplaces,
      localMarketplaces,
      plugins,
      mcpCommands,
      removedMcpServers,
    }:
    ''
      # Ensure git, ssh, and which are available for plugin marketplace operations
      # (activation PATH only includes bash, coreutils, jq, etc.)
      export PATH="${
        lib.makeBinPath (
          with pkgs;
          [
            git
            openssh
            which
          ]
        )
      }:$PATH"

      ${pkgs.coreutils}/bin/mkdir -p "$HOME/.claude"
      SETUP_LOG="$HOME/.claude/nix-setup.log"

      log() { echo "[$(date '+%H:%M:%S')] $*" >> "$SETUP_LOG"; }
      log "=== Claude Code setup started ==="

      # marketplace 조회로 인증을 확인하며, 자식 git이 /dev/tty를 열 때 SIGTTIN이 발생하지 않도록
      # timeout이 새 프로세스 그룹을 만들지 않는 foreground 모드로 실행한다.
      if ! MARKETPLACE_CACHE=$(${timeout} 15s ${claude} plugin marketplace list < /dev/null 2>/dev/null); then
        log "ERROR: Claude Code auth check failed (marketplace list timed out or errored)."
        log "  Skipping all plugin/marketplace/MCP setup."
        log "  Run 'claude auth login' to re-authenticate, then retry 'home-manager switch'."
      else

      ${mkMarketplacesSetup { inherit marketplaces localMarketplaces; }}
      ${mkPluginsSetup plugins}
      ${mkMcpSetup { inherit mcpCommands removedMcpServers; }}

      fi # end auth check

      # Fix permissions on all shell scripts in plugins (Claude Code doesn't preserve +x)
      log "Fixing plugin script permissions..."
      if [ -d "$HOME/.claude/plugins" ]; then
        find "$HOME/.claude/plugins" -type f -name "*.sh" ! -perm -111 -exec chmod +x {} + 2>/dev/null || true
        PLUGIN_SCRIPTS=$(find "$HOME/.claude/plugins" -type f -name "*.sh" 2>/dev/null | wc -l)
        log "  -> Fixed permissions for $PLUGIN_SCRIPTS shell scripts"
      fi

      log "=== Claude Code setup finished ==="
    '';

  # Plugin integration is managed separately, so install only the CLI binary.
  mkPlannotatorInstall =
    { dataHome }:
    ''
      PLANNOTATOR_BIN="$HOME/.local/bin/plannotator"
      SETUP_LOG="$HOME/.claude/nix-setup.log"
      export PATH="${
        lib.makeBinPath (
          with pkgs;
          [
            curl
            coreutils
            gnugrep
            gawk
            perl
          ]
        )
      }:$PATH"
      ${pkgs.coreutils}/bin/mkdir -p "$HOME/.claude" "$HOME/.local/bin"

      echo "[$(date '+%H:%M:%S')] Installing plannotator CLI..." >> "$SETUP_LOG"
      if (
        set -o pipefail
        ${pkgs.curl}/bin/curl -fsSL https://plannotator.ai/install.sh | ${pkgs.bash}/bin/bash -s -- --minimal
      ) >> "$SETUP_LOG" 2>&1; then
        echo "[$(date '+%H:%M:%S')] plannotator installed to $PLANNOTATOR_BIN" >> "$SETUP_LOG"
      else
        PLANNOTATOR_INSTALL_EXIT_CODE=$?
        echo "[$(date '+%H:%M:%S')] plannotator installation FAILED (exit $PLANNOTATOR_INSTALL_EXIT_CODE)" >> "$SETUP_LOG"
      fi

      # One-shot migration: remove legacy XDG-bin copy from the pre-cleanup
      # layout. No-op once cleaned up.
      LEGACY_BIN="${dataHome}/bin/plannotator"
      if [ -e "$LEGACY_BIN" ]; then
        ${pkgs.coreutils}/bin/rm -f "$LEGACY_BIN" "${dataHome}/bin/.plannotator-version"
        echo "[$(date '+%H:%M:%S')] Removed legacy $LEGACY_BIN" >> "$SETUP_LOG"
      fi
    '';
in
{
  options.modules.claude.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = "Enable Claude Code module";
  };

  options.modules.claude.mumu.enable =
    lib.mkEnableOption "the Devsisters mumu marketplace and plugin";

  options.modules.claude.otelTokenFile = lib.mkOption {
    type = lib.types.nullOr lib.types.str;
    default = null;
    description = ''
      File holding the bearer token that `claude-otel-headers` returns as the
      OTLP `Authorization` header. Null makes the helper return no headers.
    '';
  };

  config = lib.mkIf cfg.enable {
    home.packages = [
      claudePackage
    ];

    # Add ~/.local/bin to PATH (plannotator install.sh installs here)
    home.sessionPath = [
      "${config.home.homeDirectory}/.local/bin"
    ];

    home.file = {
      # Claude plugin operations may temporarily leave this as a regular file.
      ".claude/settings.json" = {
        source = mkSymlink "files/settings.json";
        force = true;
      };
      ".claude/CLAUDE.md".source = "${agentCoreOutput}/CLAUDE.md";
      ".claude/statusline-command.sh" = {
        source = ./files/statusline-command.sh;
        executable = true;
      };
      ".local/bin/claude-herdr-subagents".source = pkgs.writeShellScript "claude-herdr-subagents" ''
        exec ${pkgs.bun}/bin/bun ${herdrSubagentsSource}/modules/claude/files/herdr-subagents/index.ts "$@"
      '';
      # settings.json names this as otelHeadersHelper, so the token never
      # appears in the tracked settings file.
      ".local/bin/claude-otel-headers".source = pkgs.writeShellScript "claude-otel-headers" ''
        set -eu
        token_file=${lib.escapeShellArg (toString cfg.otelTokenFile)}
        if [ -z "$token_file" ]; then
          echo '{}'
          exit 0
        fi
        if [ ! -r "$token_file" ]; then
          echo "claude-otel-headers: token file is not readable: $token_file" >&2
          exit 1
        fi
        exec ${pkgs.jq}/bin/jq -n --rawfile token "$token_file" \
          '{ Authorization: ("Bearer " + ($token | rtrimstr("\n"))) }'
      '';
    };

    home.activation.setupClaudeCode = lib.hm.dag.entryAfter [ "writeBoundary" ] (mkClaudeCodeSetup {
      inherit
        marketplaces
        localMarketplaces
        plugins
        mcpCommands
        removedMcpServers
        ;
    });
    home.activation.installPlannotator =
      lib.hm.dag.entryAfter [ "writeBoundary" ]
        (mkPlannotatorInstall {
          dataHome = config.xdg.dataHome;
        });
  };
}
