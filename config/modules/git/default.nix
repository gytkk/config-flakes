{ config, lib, ... }:

let
  cfg = config.modules.git;
in
{
  options.modules.git.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = "Enable Git module";
  };

  config = lib.mkIf cfg.enable {
    programs.delta = {
      enable = true;
      enableGitIntegration = true;
      options = {
        syntax-theme = "ansi";
        navigate = true;
        side-by-side = true;
        line-numbers = true;
        light = true;
      };
    };

    programs.git = {
      enable = true;

      lfs.enable = true;

      settings = {
        user = {
          name = "gytkk";
          email = "gytk.kim@gmail.com";
        };

        core = {
          editor = "nvim";
          excludesfile = "~/.gitignore";
          quotepath = "false";
        };

        # Color configuration
        color = {
          ui = "auto";
          diff = "always";
          status = "auto";
          branch = "auto";
        };

        "color \"diff\"" = {
          old = "red";
          new = "green";
        };

        # Pull configuration
        pull = {
          rebase = true;
          ff = "only";
        };

        push = {
          default = "current";
          autoSetupRemote = true;
        };

        # Init configuration
        init = {
          defaultBranch = "main";
        };

        fetch = {
          all = true;
          prune = true;
          pruneTags = true;
        };

        credential = {
          "https://github.com".helper = [
            ""
            "!gh auth git-credential"
          ];
          "https://gist.github.com".helper = [
            ""
            "!gh auth git-credential"
          ];
        };

        filter.codex-clean = {
          clean = "sed '/^\\[projects\\./,/^$/d'";
          smudge = "cat";
        };

      };
    };

    # Create .gitignore file
    home.file.".gitignore".text = ''
      .DS_Store
      .idea
      .vscode
      .zed
      .tool-versions
      .ai

      # Environment
      .direnv/
      .env
      .envrc
      mise.toml

      # Python
      __pycache__/
      *.py[codz]
      *$py.class.Python

      build/
      develop-eggs/
      dist/
      downloads/
      eggs/
      .eggs/
      parts/
      sdist/
      wheels/
      share/python-wheels/
      *.egg-info/
      .installed.cfg
      *.egg
      MANIFEST

      .coverage
      htmlcov/

      .ruff_cache/

      # Node.js
      logs
      *.log
      npm-debug.log*
      yarn-debug.log*
      yarn-error.log*
      lerna-debug.log*

      # Nix
      result
      result-*

      # Claude
      .claude/settings.local.json

      # Sisyphus planning files
      .sisyphus/
    '';
  };
}
