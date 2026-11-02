{
  config,
  pkgs,
  lib,
  flakeDirectory,
  ...
}:

let
  cfg = config.modules.claude;
  agentCoreOutput = import ../../../agent-core/nix/render.nix { inherit pkgs; } {
    runtime = "claude";
  };
  herdrSubagentsSource = lib.fileset.toSource {
    root = ../..;
    fileset = lib.fileset.unions [
      ./files/herdr-subagents
      ../herdr/files/jsonl-tail.ts
    ];
  };
  claude = "${pkgs.claude-code}/bin/claude";
  mkSymlink =
    path: config.lib.file.mkOutOfStoreSymlink "${flakeDirectory}/config/modules/claude/${path}";
  marketplaces = [
    "anthropics/skills"
    "anthropics/claude-code"
    "anthropics/claude-plugins-official"
    "backnotprop/plannotator"
    "openai/codex-plugin-cc"
    "DietrichGebert/ponytail"
    "pbakaus/impeccable"
  ]
  ++ lib.optional cfg.mumu.enable "devsisters/mumu";

  # Local marketplaces sourced from the flake working tree.
  # Each entry is registered via `claude plugin marketplace add <path>`.
  localMarketplaces = [
    {
      name = "gytkk";
      path = "${flakeDirectory}/config/modules/claude/marketplace";
    }
  ];

  # plugin-name@marketplace-name
  plugins = [
    "document-skills@anthropic-agent-skills"
    "commit-commands@claude-code-plugins"
    "security-guidance@claude-code-plugins"

    "ralph-loop@claude-plugins-official"
    "superpowers@claude-plugins-official"
    "ponytail@ponytail"
    "impeccable@impeccable"

    "slack@claude-plugins-official"

    "gopls-lsp@claude-plugins-official"
    "rust-analyzer-lsp@claude-plugins-official"
    "typescript-lsp@claude-plugins-official"

    # backnotprop/plannotator — visual plan annotation and review
    "plannotator@plannotator"

    # openai/codex-plugin-cc — Official Codex plugin for Claude Code
    "codex@openai-codex"

    # local gytkk marketplace (modules/claude/marketplace) - runtime-specific skill and LSP plugins
    "devils-advocate@gytkk"
    "ty-lsp@gytkk"
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
    "playwright"
  ];

  mkClaudeCodeSetup = import ./activation.nix { inherit pkgs lib claude; };

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
