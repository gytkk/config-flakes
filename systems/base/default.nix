{
  config,
  lib,
  pkgs,
  username,
  homeDirectory,
  flakeDirectory,
  inputs,
  isWSL ? false,
  osConfig ? null,
  ...
}:

let
  agenixPackage = inputs.agenix.packages.${pkgs.stdenv.hostPlatform.system}.default;
  agx = pkgs.writeShellScriptBin "agx" ''
    set -eu

    secretsDir=${lib.escapeShellArg "${flakeDirectory}/secrets"}
    identity="$HOME/.ssh/id_ed25519"

    if [ ! -r "$identity" ]; then
      echo "agx: identity is not readable: $identity" >&2
      exit 1
    fi

    cd "$secretsDir"
    export RULES="$secretsDir/secrets.nix"
    exec ${agenixPackage}/bin/agenix "$@" -i "$identity"
  '';

in
{
  imports = [
    ./compat/home-manager.nix
    ../../modules/claude-session-mining
    # 기본 모듈들 (항상 import됨)
    ../../modules/agent-skills
    ../../modules/agent-session-record
    ../../modules/aerospace
    ../../modules/claude
    ../../modules/codex
    ../../modules/ghostty
    ../../modules/git
    ../../modules/gksdud
    ../../modules/herdr
    ../../modules/herdr-app
    ../../modules/hunk
    ../../modules/k9s
    ../../modules/lsp
    ../../modules/nix-gc
    ../../modules/openai-api
    ../../modules/pi
    ../../modules/tmux
    ../../modules/vim
    ../../modules/windows-terminal
    ../../modules/zed
    ../../modules/zsh
  ];

  options.modules.commonTheme = lib.mkOption {
    type = lib.types.str;
    default = "one-half-light";
    description = "Canonical theme id shared across theme-aware app modules.";
    example = "catppuccin-mocha";
  };

  config = {
    modules = {
      agentSessionRecord.enable = lib.mkDefault true;
      aerospace.enable = lib.mkDefault pkgs.stdenv.hostPlatform.isDarwin;
      claude.enable = lib.mkDefault true;
      codex.enable = lib.mkDefault true;
      ghostty.enable = lib.mkDefault true;
      git.enable = lib.mkDefault true;
      gksdud.enable = lib.mkDefault pkgs.stdenv.hostPlatform.isDarwin;
      herdr.enable = lib.mkDefault true;
      herdrApp.enable = lib.mkDefault (pkgs.stdenv.hostPlatform.isDarwin || isWSL);
      hunk.enable = lib.mkDefault true;
      openaiApi.enable = lib.mkDefault (
        config.modules.vim.enable || config.modules.codex.enable || config.modules.claude.enable
      );
      k9s.enable = lib.mkDefault true;
      lsp.enable = lib.mkDefault true;
      # NixOS hosts collect garbage at the system level via modules/nixos/baseline.nix.
      nixGc.enable = lib.mkDefault (osConfig == null);
      pi.enable = lib.mkDefault true;
      tmux.enable = lib.mkDefault true;
      vim.enable = lib.mkDefault true;
      windowsTerminal.enable = lib.mkDefault true;
      zed.enable = lib.mkDefault true;
      zsh.enable = lib.mkDefault true;
    };

    # Disable news on update
    news.display = "silent";

    # services.home-manager.autoExpire is deliberately left off: it only expires
    # the home-manager profile, while each switch also adds a generation to the
    # nix-env profile beside it. modules/nix-gc prunes every profile instead.

    age.identityPaths = [ "${homeDirectory}/.ssh/id_ed25519" ];

    # XDG Base Directory Specification
    xdg = {
      enable = true;
      configHome = "${homeDirectory}/.config";
    };

    home = {
      inherit username homeDirectory;

      # Set language for shell sessions managed by home-manager
      language = {
        base = if isWSL then "C.UTF-8" else "ko_KR.UTF-8";
      };

      # Keep legacy nix profile installs visible in interactive shells.
      sessionPath = [
        "$HOME/.nix-profile/bin"
      ];

      # Keep desktop and non-interactive SSH discovery on the Nix-managed Core.
      file.".local/bin/herdr-core".source = "${pkgs.herdr-core}/bin/herdr-core";
      file.".local/bin/hc".source = "${pkgs.herdr-core}/bin/hc";

      # 기본 패키지 (모든 환경에서 공통)
      packages = with pkgs; [
        # Nix
        agent-core
        nixfmt

        # System utilities
        bashInteractive
        coreutils
        findutils
        gnupg
        libiconv

        # Development (common)
        docker
        gcc

        # Dev tools
        awscli2
        cf
        jq
        sqlite
        yq-go # yq 패키지는 더 이상 관리되지 않음
        fd
        ripgrep
        less
        wget
        curl
        rsync
        glow
        ntn
        datadog-pup

        # Git
        git
        gh
        lazygit
        gitui
        hunk
        delta
        bat

        # Python
        python3
        uv
        ruff

        # Rust
        rustup

        # JavaScript + Node.js
        bun
        pnpm
        nodejs
        typescript

        # Go
        go

        # Kubernetes
        kubectl
        kubectx
        kubernetes-helm

        # Secrets
        _1password-cli
        keybase
        agenixPackage
        agx

        # Modeling
        ffmpeg
        imagemagick
        yt-dlp

        # AI usage
        codexbar
        herdr-core

        # etc
        direnv

        # Fonts
        nerd-fonts.fira-code
        nerd-fonts.jetbrains-mono
        nanum-gothic-coding
        sarasa-gothic
        noto-fonts-cjk-sans
        noto-fonts-cjk-serif
      ];

      stateVersion = "26.05";

      # Rust/C 빌드 시 라이브러리 경로 (특히 macOS에서 libiconv 링킹용)
      sessionVariables = {
        LIBRARY_PATH = lib.makeLibraryPath [ pkgs.libiconv ];

        # rebuild마다 pup의 코드 서명이 바뀌어 macOS 키체인 알림이 반복되므로
        # 토큰을 ~/.config/pup/tokens_<site>.json에 0600 권한으로 저장한다.
        DD_TOKEN_STORAGE = "file";
      };
    };

    programs = {
      # Enable Home Manager
      home-manager = {
        enable = true;
      };

      direnv = {
        enable = true;
        enableZshIntegration = true;
        nix-direnv.enable = true;
      };
    };

  };
}
