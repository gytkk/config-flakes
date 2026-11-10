{
  config,
  lib,
  pkgs,
  themeExports,
  isWSL ? false,
  osConfig ? null,
  ...
}:

let
  cfg = config.modules.zsh;
  isPylvOnyx = osConfig != null && (osConfig.networking.hostName or null) == "pylv-onyx";
  locale = if isWSL then "C.UTF-8" else "en_US.UTF-8";
  nixCommandTiming = pkgs.writeShellScriptBin "nix-command-timing" ''
    exec ${pkgs.python3}/bin/python ${./nix-command-timing.py} "$@"
  '';
  gitCommitMessage = pkgs.writeShellScriptBin "git-commit-message" ''
    exec with-openai ${pkgs.bun}/bin/bun ${./plugins/git-commit-suggestions/git-commit-message.ts} "$@"
  '';
in
let
  starshipThemeConfig = config.lib.file.mkOutOfStoreSymlink (
    themeExports.mutableFile "starship" "${config.modules.commonTheme}.toml"
  );
in
{
  options.modules.zsh.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = "Enable Zsh module";
  };

  config = lib.mkIf cfg.enable {
    home.packages =
      with pkgs;
      [
        zsh
        nixCommandTiming
      ]
      ++ lib.optionals config.modules.openaiApi.enable [ gitCommitMessage ];

    # Zsh configuration
    programs.zsh = {
      enable = true;

      # Use XDG config directory for zsh configuration files
      dotDir = "${config.xdg.configHome}/zsh";

      # macOS path_helper runs after Home Manager's session PATH setup.
      profileExtra = lib.optionalString pkgs.stdenv.hostPlatform.isDarwin ''
        path=("$HOME/.nix-profile/bin" "''${(@)path:#$HOME/.nix-profile/bin}")
      '';

      # Enable features
      autosuggestion.enable = true;
      enableCompletion = true;
      syntaxHighlighting.enable = true;

      plugins = lib.optionals config.modules.openaiApi.enable [
        {
          name = "git-commit-suggestions";
          src = ./plugins/git-commit-suggestions;
          file = "git-commit-suggestions.plugin.zsh";
        }
      ];

      # History
      history = {
        size = 10000;
        save = 10000;
        path = "${config.home.homeDirectory}/.zsh_history";
        ignoreDups = true;
        share = true;
        extended = true;
      };

      shellAliases = {
        # Home manager aliases
        hm = "home-manager";
        hmb = "home-manager build";
        hms = "home-manager switch";

        # ls aliases with color by default
        ls = "ls --color=auto";
        ll = "ls -la --color=auto";
        lh = "ls -lh --color=auto";

        # git
        ga = "git add";
        gb = "git branch";
        gc = "git commit";
        gp = "git push";
        gl = "git pull";
        gst = "git status";
        gsw = "git switch";
        gco = "git checkout";
        gd = "git diff";
        lg = "lazygit";

        # Neovim
        vim = "nvim";
        vi = "nvim";
        vimdiff = "nvim -d";

        # k8s aliases
        kl = "kubectl";
        kx = "kubectx";
        kn = "kubens";
      }
      // lib.optionalAttrs (!pkgs.stdenv.hostPlatform.isDarwin) {
        zed = if isWSL then "zed.exe" else "zeditor";
      };

      # Environment variables
      sessionVariables = {
        SHELL = "${pkgs.zsh}/bin/zsh";
        EDITOR = "nvim";
        VISUAL = "nvim";
        COLORTERM = "truecolor";
        LANG = locale;
        LC_CTYPE = locale;
      };

      # Zsh initialization
      # https://nix-community.github.io/home-manager/options.xhtml#opt-programs.zsh.initContent
      initContent =
        let
          earlyInit = lib.mkOrder 500 ''
            # Load Nix if not ready
            if [[ ! $(command -v nix) && -e '/nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh' ]]; then
              source '/nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh'
            fi

            # Prevent LC_ALL from overriding LANG/LC_CTYPE
            unset LC_ALL

            # Let headless shells discover user services through `systemctl --user`.
            if [[ -z "''${XDG_RUNTIME_DIR:-}" ]]; then
              export XDG_RUNTIME_DIR="/run/user/$(id -u)"
            fi
            if [[ -z "''${DBUS_SESSION_BUS_ADDRESS:-}" && -S "$XDG_RUNTIME_DIR/bus" ]]; then
              export DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus"
            fi
          '';

          zshConfig = lib.mkOrder 1000 ''
            ${builtins.readFile ./nix-command-timing.zsh}

            # gh auth 후 자동으로 ~/.netrc 업데이트 (Nix flake용)
            gh-auth-netrc() {
              gh auth "$@"
              if [[ $? -eq 0 ]] && gh auth status &>/dev/null; then
                local token=$(gh auth token 2>/dev/null)
                if [[ -n "$token" ]]; then
                  # 기존 github.com 항목 제거 후 새로 추가
                  grep -v "machine github.com" ~/.netrc > ~/.netrc.tmp 2>/dev/null || touch ~/.netrc.tmp
                  echo "machine github.com login oauth password $token" >> ~/.netrc.tmp
                  mv ~/.netrc.tmp ~/.netrc
                  chmod 600 ~/.netrc
                  echo "Updated ~/.netrc with GitHub token for Nix"
                fi
              fi
            }

            # Remove the worktree holding a branch, then switch to it here.
            gsww() {
              if [[ $# -ne 1 ]]; then
                echo "Usage: gsww <branch>" >&2
                return 2
              fi

              local branch="$1"
              local target_ref="refs/heads/$branch"
              local current_path worktree_path candidate record

              current_path="$(git rev-parse --show-toplevel 2>/dev/null)" || {
                echo "gsww: not inside a Git worktree" >&2
                return 1
              }

              for record in "''${(@0)$(git worktree list --porcelain -z)}"; do
                case "$record" in
                  "worktree "*) candidate="''${record#worktree }" ;;
                  "branch $target_ref")
                    worktree_path="$candidate"
                    break
                    ;;
                esac
              done

              if [[ -n "$worktree_path" && "$worktree_path" != "$current_path" ]]; then
                git worktree remove "$worktree_path" || return
              fi

              git switch "$branch"
            }

            # History prefix search with up/down arrows
            autoload -U up-line-or-beginning-search down-line-or-beginning-search
            zle -N up-line-or-beginning-search
            zle -N down-line-or-beginning-search
            # ^[[A/^[[B: normal mode (macOS, some Linux terminals)
            # ^[OA/^[OB: application mode (Linux, WSL, tmux)
            bindkey "^[[A" up-line-or-beginning-search
            bindkey "^[[B" down-line-or-beginning-search
            bindkey "^[OA" up-line-or-beginning-search
            bindkey "^[OB" down-line-or-beginning-search

            # Home/End key bindings (for vi mode compatibility)
            # ^[[H/^[[F: normal mode, ^[OH/^[OF: application mode
            bindkey "^[[H" beginning-of-line
            bindkey "^[[F" end-of-line
            bindkey "^[OH" beginning-of-line
            bindkey "^[OF" end-of-line
            bindkey -M vicmd "^[[H" beginning-of-line
            bindkey -M vicmd "^[[F" end-of-line
            bindkey -M vicmd "^[OH" beginning-of-line
            bindkey -M vicmd "^[OF" end-of-line

            # Enable colors
            autoload -U colors && colors

            # Set zsh completion to use LS_COLORS
            zstyle ':completion:*' list-colors "$LS_COLORS"

            # Set uv shell completion
            if command -v uv > /dev/null; then
              eval "$(uv generate-shell-completion zsh)"
            fi
          '';

          openclawCompletion = lib.mkIf isPylvOnyx (
            lib.mkOrder 1100 ''
              # OpenClaw completion is managed declaratively here so the CLI's
              # one-shot installer does not get reverted by Home Manager.
              if command -v openclaw > /dev/null; then
                local_openclaw_completion_cache="$HOME/.openclaw/completions/openclaw.zsh"
                if [[ ! -r "$local_openclaw_completion_cache" ]]; then
                  openclaw completion --write-state >/dev/null 2>&1 || true
                fi
                # OpenClaw Completion
                [[ -r "$local_openclaw_completion_cache" ]] && source "$local_openclaw_completion_cache"
                unset local_openclaw_completion_cache
              fi
            ''
          );
        in
        lib.mkMerge [
          earlyInit
          zshConfig
          openclawCompletion
        ];
    };

    # Starship prompt
    programs.starship = {
      enable = true;
      enableZshIntegration = true;
    };

    # Starship configuration from generated canonical theme export
    xdg.configFile."starship.toml".source = starshipThemeConfig;

    # dircolors for LS_COLORS
    programs.dircolors = {
      enable = true;
      enableZshIntegration = true;
    };

    # zoxide (replacement for z)
    programs.zoxide = {
      enable = true;
      enableZshIntegration = true;
    };

    # fzf configuration
    programs.fzf = {
      enable = true;
      enableZshIntegration = true;
    };

    # direnv configuration
    programs.direnv = {
      enable = true;
      enableZshIntegration = true;
      nix-direnv.enable = true;
    };
  };
}
