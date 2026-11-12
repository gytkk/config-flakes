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
  lspExtension = pkgs.applyPatches {
    name = "pi-lsp-refactor-0.1.7";
    src = pkgs.fetchzip {
      url = "https://registry.npmjs.org/pi-lsp/-/pi-lsp-0.1.7.tgz";
      sha256 = "12fhprz6y7i5skrf136b6ip7jnz930ms18cm5gci00m025zyb64b";
    };
    patches = [ ./lsp-refactor.patch ];
    postPatch = ''
      cp -r ${./files/extensions/lsp-refactor} extensions/pi-lsp/refactor
      echo 'export { default } from "./pi-lsp/index";' > extensions/index.ts
    '';
  };
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
    ];

    home.file = {
      ".pi/agent/AGENTS.md".source = "${agentCoreOutput}/AGENTS.md";
      ".pi/agent/APPEND_SYSTEM.md".source = "${agentCoreOutput}/APPEND_SYSTEM.md";
      ".pi/agent/agents/researcher.md".source = mkSymlink "agents/researcher.md";
      ".pi/agent/agents/reviewer.md".source = mkSymlink "agents/reviewer.md";
      ".pi/agent/lsp.json".source = mkSymlink "files/lsp.json";
      ".pi/agent/mcp.json".source = mkSymlink "files/mcp.json";
      ".pi/agent/models.json".source = mkSymlink "files/models.json";
      ".pi/web-search.json".source = mkSymlink "files/web-search.json";
      ".pi/agent/themes/claude-like.json".source = mkSymlink "files/themes/claude-like.json";
      ".pi/agent/themes/one-half-light.json".source = themeExports.file "pi" "one-half-light.json";
      ".pi/agent/extensions/codex-fast-mode.ts".source = mkSymlink "files/extensions/codex-fast-mode.ts";
      ".pi/agent/extensions/codex-usage.ts".source = mkSymlink "files/extensions/codex-usage.ts";
      ".pi/agent/extensions/pi-lsp".source = "${lspExtension}/extensions";
      ".pi/agent/extensions/hardware-cursor-only.ts".source =
        mkSymlink "files/extensions/hardware-cursor-only.ts";
      ".pi/agent/extensions/herdr-agent-state.ts" = lib.mkIf config.modules.herdr.enable {
        source = "${pkgs.herdr.src}/src/integration/assets/pi/herdr-agent-state.ts";
      };
      ".pi/agent/extensions/herdr-subagents.ts".source = mkSymlink "files/extensions/herdr-subagents.ts";
      ".pi/agent/extensions/terminal-title-reset.ts".source =
        mkSymlink "files/extensions/terminal-title-reset.ts";
      ".pi/agent/extensions/subagent/config.json".source =
        mkSymlink "files/extensions/subagent/config.json";
    };

    # Preserve installation-local metadata before Home Manager removes the old symlink.
    home.activation.piSettings = lib.hm.dag.entryBetween [ "linkGeneration" ] [ "writeBoundary" ] ''
      run ${syncSettings} ${./files/settings.json} "$HOME/.pi/agent/settings.json"
    '';
  };
}
