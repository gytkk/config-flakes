{
  config,
  pkgs,
  lib,
  flakeDirectory,
  hasSystemCodexConfig ? false,
  ...
}:

let
  cfg = config.modules.codex;
  agentCoreOutput = import ../../agent-core/nix/render.nix { inherit pkgs; } {
    runtime = "codex";
  };
  herdrSubagentsSource = lib.fileset.toSource {
    root = ../..;
    fileset = lib.fileset.unions [
      ./files/herdr-subagents
      ../herdr/files/jsonl-tail.ts
    ];
  };
  codex = "${pkgs.codex}/bin/codex";
  codexLauncher = pkgs.writeShellScriptBin "codex" (
    builtins.replaceStrings [ "@codex@" ] [ (lib.escapeShellArg codex) ] (
      builtins.readFile ./files/codex-launcher.sh
    )
  );
  lspMcp = pkgs.writeShellScriptBin "codex-lsp-mcp" ''
    exec ${pkgs.lspi}/bin/lspi mcp \
      --workspace-root "$PWD" \
      --config ${./files/lspi.toml} \
      --read-only "$@"
  '';
  hookCommandNames = [
    "agent-session-record"
    "plannotator"
    "codex-herdr-subagents"
  ];
  codexHooksJson = builtins.replaceStrings (map (name: "~/.local/bin/${name}") hookCommandNames) (map
    (name: "${config.home.homeDirectory}/.local/bin/${name}")
    hookCommandNames
  ) (builtins.readFile ./files/hooks.json);
  codexConfigPath = "${config.home.homeDirectory}/.codex/config.toml";
  coreutils = pkgs.coreutils;
  systemCodexConfigActivation = lib.optionalString (!hasSystemCodexConfig) (
    import ./system-activation.nix {
      inherit lib coreutils;
      configSource = "${flakeDirectory}/modules/codex/files/config.toml";
    }
  );
  codexUserConfigActivation = ''
    ${coreutils}/bin/mkdir -p "$HOME/.codex"

    if [ ! -e ${lib.escapeShellArg codexConfigPath} ]; then
      : > ${lib.escapeShellArg codexConfigPath}
      ${coreutils}/bin/chmod 600 ${lib.escapeShellArg codexConfigPath}
    fi
  '';
in
{
  options.modules.codex = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Enable Codex CLI module";
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [
      (lib.hiPrio codexLauncher)
      pkgs.codex
      pkgs.mcp-nixos
      pkgs.lspi
      lspMcp
    ];

    home.file.".codex/AGENTS.md".source = "${agentCoreOutput}/AGENTS.md";
    home.file.".codex/hooks.json".text = codexHooksJson;
    home.file.".local/bin/codex-herdr-subagents".source =
      pkgs.writeShellScript "codex-herdr-subagents" ''
        exec ${pkgs.bun}/bin/bun ${herdrSubagentsSource}/modules/codex/files/herdr-subagents/index.ts "$@"
      '';

    home.activation.codexUserConfig = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
      ${systemCodexConfigActivation}

      ${codexUserConfigActivation}
    '';

    home.activation.setupCodexPlugins = lib.hm.dag.entryAfter [ "codexUserConfig" ] ''
      export PATH="${lib.makeBinPath (with pkgs; [ git ])}:$PATH"

      ${coreutils}/bin/mkdir -p "$HOME/.codex"
      SETUP_LOG="$HOME/.codex/nix-setup.log"
      LEGACY_SUPERPOWERS_SKILLS="$HOME/.codex/superpowers/skills"
      SUPERPOWERS_SKILL_LINK="$HOME/.agents/skills/superpowers"

      log() { echo "[$(${coreutils}/bin/date '+%H:%M:%S')] $*" >> "$SETUP_LOG"; }

      cleanup_legacy_skill_link() {
        if [ -L "$SUPERPOWERS_SKILL_LINK" ]; then
          current_target="$(${coreutils}/bin/readlink "$SUPERPOWERS_SKILL_LINK")"
          if [ "$current_target" = "$LEGACY_SUPERPOWERS_SKILLS" ]; then
            run ${coreutils}/bin/rm -f "$SUPERPOWERS_SKILL_LINK"
          fi
        fi
      }

      log "=== Codex plugin setup started ==="
      log "Installing plugin: superpowers@openai-curated"
      if run ${codex} plugin add superpowers@openai-curated < /dev/null >> "$SETUP_LOG" 2>&1; then
        cleanup_legacy_skill_link
        log "  -> OK"
      else
        log "  -> FAILED (exit $?)"
        errorEcho "Could not install superpowers@openai-curated. See $SETUP_LOG and retry home-manager switch."
        exit 1
      fi
      log "Ensuring plugin is installed: ponytail@ponytail"
      if run ${pkgs.bash}/bin/bash ${./files/install-ponytail.sh} ${codex} ${pkgs.jq}/bin/jq < /dev/null >> "$SETUP_LOG" 2>&1; then
        log "  -> OK"
      else
        log "  -> FAILED (exit $?)"
        errorEcho "Could not check or install ponytail@ponytail. See $SETUP_LOG and retry home-manager switch."
        exit 1
      fi
      log "=== Codex plugin setup finished ==="
    '';
  };
}
