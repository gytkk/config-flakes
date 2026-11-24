{
  config,
  lib,
  pkgs,
  ...
}:
let
  enabled = path: lib.attrByPath ([ "modules" ] ++ path ++ [ "enable" ]) false config;
  targets =
    lib.optional (enabled [ "codex" ] || enabled [ "pi" ]) "shared"
    ++ lib.optional (enabled [ "claude" ]) "claude";
  manifest = builtins.fromTOML (builtins.readFile ../../agent-core/manifest.toml);
  sharedLinks = lib.mergeAttrsList (
    lib.mapAttrsToList (
      _: source:
      lib.mapAttrs' (
        name: path:
        lib.nameValuePair ".agents/skills/${name}" {
          source = ../../agent-core + "/${source.path}/${path}";
        }
      ) (lib.filterAttrs (name: _: builtins.elem name manifest.skill_targets.shared.skills) source.skills)
    ) (lib.filterAttrs (_: source: source.kind == "local") manifest.skill_sources)
  );
  python = pkgs.python3.withPackages (ps: [ (ps.toPythonModule pkgs.agent-core) ]);
  installer = pkgs.writeShellScript "install-agent-skills" ''
    export PATH="${
      lib.makeBinPath [
        pkgs.nodejs
        pkgs.git
      ]
    }:$PATH"
    exec ${python}/bin/python ${../../scripts/install-agent-skills.py} "$@"
  '';
in
{
  config = lib.mkIf (targets != [ ]) {
    home.file = lib.optionalAttrs (builtins.elem "shared" targets) sharedLinks;
    home.activation.installAgentSkills =
      lib.hm.dag.entryAfter ([ "linkGeneration" ] ++ lib.optional (enabled [ "codex" ]) "codexUserConfig")
        ''
          if run ${pkgs.coreutils}/bin/env \
            XDG_STATE_HOME=${lib.escapeShellArg config.xdg.stateHome} \
            ${installer} install --apply ${
              lib.concatMapStringsSep " " (target: "--target ${target}") targets
            }; then
            :
          else
            agentSkillsExitCode=$?
            errorEcho "Agent skills installation failed. Review the installer output above and retry home-manager switch."
            exit "$agentSkillsExitCode"
          fi
        '';
  };
}
