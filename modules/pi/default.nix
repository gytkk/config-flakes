{
  config,
  lib,
  pkgs,
  flakeDirectory,
  themeExports,
  ...
}:

let
  cfg = config.modules.pi;
  agentCoreOutput = import ../../agent-core/nix/render.nix { inherit pkgs; } { runtime = "pi"; };
  mkSymlink = path: config.lib.file.mkOutOfStoreSymlink "${flakeDirectory}/modules/pi/${path}";
  lspExtension = import ./extensions/pi-lsp.nix { inherit pkgs; };

  extensionPackages = [
    "npm:@juicesharp/rpiv-ask-user-question@2.15.0"
    {
      source = "npm:pi-lsp@0.1.7";
      extensions = [ ]; # Keep npm dependencies; load only the patched local extension below.
    }
    {
      source = "npm:pi-interactive-shell@0.18.0";
      skills = [ ];
      prompts = [ ];
    }
    "npm:pi-web-access@0.46.0"
    {
      source = "npm:pi-subagents@0.84.0";
      skills = [ ];
      prompts = [ ];
    }
  ];

  localExtensions = {
    "codex-fast-mode.ts" = mkSymlink "files/extensions/codex-fast-mode.ts";
    "codex-usage.ts" = mkSymlink "files/extensions/codex-usage.ts";
    "rounded-tools.ts" = mkSymlink "files/extensions/rounded-tools.ts";
    "pi-lsp" = "${lspExtension}/extensions";
    "prompt-ui" = mkSymlink "files/extensions/prompt-ui";
    "phase-tasks" = mkSymlink "files/extensions/phase-tasks";
    "python-kernel" = mkSymlink "files/extensions/python-kernel";
    "ast-transform" = mkSymlink "files/extensions/ast-transform";
    "session-artifacts" = mkSymlink "files/extensions/session-artifacts";
    "agent-hub" = mkSymlink "files/extensions/agent-hub";
    "herdr-subagents.ts" = mkSymlink "files/extensions/herdr-subagents.ts";
    "terminal-title-reset.ts" = mkSymlink "files/extensions/terminal-title-reset.ts";
  }
  // lib.optionalAttrs config.modules.herdr.enable {
    "herdr-agent-state.ts" = "${pkgs.herdr.src}/src/integration/assets/pi/herdr-agent-state.ts";
  };

  extensionFiles = lib.mapAttrs' (
    name: source: lib.nameValuePair ".pi/agent/extensions/${name}" { inherit source; }
  ) localExtensions;
  commonSettings = pkgs.writeText "pi-common-settings.json" (
    builtins.toJSON (
      builtins.fromJSON (builtins.readFile ./files/settings.json) // { packages = extensionPackages; }
    )
  );
  syncSettings = pkgs.writeShellScript "pi-sync-settings" ''
    export PATH=${
      lib.makeBinPath (
        with pkgs;
        [
          coreutils
          jq
        ]
      )
    }:$PATH
    exec ${pkgs.bash}/bin/bash ${./files/sync-settings.sh} "$@"
  '';
in
{
  options.modules.pi.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = "Enable Pi coding agent CLI";
  };

  config = lib.mkIf cfg.enable {
    home.packages = [
      pkgs.pi
      pkgs.mcp-nixos
      pkgs.python3
      pkgs.ast-grep
    ];

    home.file = extensionFiles // {
      ".pi/agent/AGENTS.md".source = "${agentCoreOutput}/AGENTS.md";
      ".pi/agent/APPEND_SYSTEM.md".source = "${agentCoreOutput}/APPEND_SYSTEM.md";
      ".pi/agent/agents/researcher.md".source = mkSymlink "agents/researcher.md";
      ".pi/agent/agents/reviewer.md".source = mkSymlink "agents/reviewer.md";
      ".pi/agent/agents/worker.md".source = mkSymlink "agents/worker.md";
      ".pi/agent/interactive-shell.json".source = mkSymlink "files/interactive-shell.json";
      ".pi/agent/lsp.json".source = mkSymlink "files/lsp.json";
      ".pi/agent/mcp.json".source = mkSymlink "files/mcp.json";
      ".pi/agent/models.json".source = mkSymlink "files/models.json";
      ".pi/web-search.json".source = mkSymlink "files/web-search.json";
      ".pi/agent/themes/claude-like.json".source = mkSymlink "files/themes/claude-like.json";
      ".pi/agent/themes/one-half-light.json".source = themeExports.file "pi" "one-half-light.json";
      ".pi/agent/extensions/subagent/config.json".source =
        mkSymlink "files/extensions/subagent/config.json";
    };

    # Preserve installation-local metadata before Home Manager removes the old symlink.
    home.activation.piSettings = lib.hm.dag.entryBetween [ "linkGeneration" ] [ "writeBoundary" ] ''
      run ${syncSettings} ${commonSettings} "$HOME/.pi/agent/settings.json"
    '';
  };
}
