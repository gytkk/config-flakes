{
  config,
  lib,
  pkgs,
  inputs,
  isWSL ? false,
  ...
}:

let
  cfg = config.modules.vscode;

  # nix-vscode-extensions에서 시스템에 맞는 확장 가져오기 (nixpkgs에 없는 것만)
  marketplaceExtensions =
    inputs.nix-vscode-extensions.extensions.${pkgs.stdenv.hostPlatform.system}.vscode-marketplace;

  # 로컬 One Half Light 테마 확장
  oneHalfLightTheme = pkgs.stdenv.mkDerivation {
    pname = "vscode-one-half-light-theme";
    version = "1.0.0";
    src = ./one-half-light-theme;
    installPhase = ''
      runHook preInstall
      mkdir -p $out/share/vscode/extensions/local.one-half-light-theme
      cp -r . $out/share/vscode/extensions/local.one-half-light-theme/
      runHook postInstall
    '';
    passthru = {
      vscodeExtUniqueId = "local.one-half-light-theme";
      vscodeExtPublisher = "local";
      vscodeExtName = "one-half-light-theme";
    };
  };

  # nixpkgs 확장 (빠름, 캐시됨)
  nixpkgsExtensions = with pkgs.vscode-extensions; [
    # AI (copilot은 nixpkgs에서)
    github.copilot
    github.copilot-chat

    # Docker/Kubernetes
    ms-azuretools.vscode-docker
    ms-azuretools.vscode-containers
    ms-kubernetes-tools.vscode-kubernetes-tools
    ms-vscode-remote.remote-containers

    # Git/GitHub
    github.vscode-github-actions
    eamodio.gitlens

    # Go
    golang.go

    # JavaScript/TypeScript
    bradlc.vscode-tailwindcss
    dbaeumer.vscode-eslint
    prisma.prisma

    # Localization
    ms-ceintl.vscode-language-pack-ko

    # Nix
    jnoortheen.nix-ide

    # Python
    charliermarsh.ruff
    ms-python.debugpy
    ms-python.python
    ms-python.vscode-pylance

    # Remote
    ms-vscode-remote.remote-wsl

    # Rust
    rust-lang.rust-analyzer

    # Scala
    scala-lang.scala

    # Terraform
    hashicorp.terraform

    # Tools
    ms-vscode.makefile-tools

    # Vim
    vscodevim.vim

    # YAML/TOML
    redhat.vscode-yaml
    tamasfe.even-better-toml
  ];

  # nix-vscode-extensions에서만 가져올 확장 (nixpkgs에 없거나 자주 업데이트되는 것들)
  extraExtensions = with marketplaceExtensions; [
    # Theme
    oneHalfLightTheme

    # Python
    astral-sh.ty
    ms-python.vscode-python-envs
  ];

  # 공통 확장 프로그램 (macOS/Linux/WSL 모두 사용)
  commonExtensions = nixpkgsExtensions ++ extraExtensions;

  # WSL용 확장 심볼릭 링크 생성
  # Nix 패키지 구조: ${ext}/share/vscode/extensions/${ext.vscodeExtUniqueId}/
  wslExtensionLinks = builtins.listToAttrs (
    map (ext: {
      name = ".vscode-server/extensions/${ext.vscodeExtUniqueId}";
      value = {
        source = "${ext}/share/vscode/extensions/${ext.vscodeExtUniqueId}";
      };
    }) commonExtensions
  );

  # WSL용 settings.json 생성
  wslSettingsFile = pkgs.writeText "vscode-settings.json" (builtins.toJSON userSettings);

  # WSL용 extensions.json 항목 생성
  # VSCode가 확장을 인식하려면 extensions.json에 등록되어야 함
  mkExtensionEntry = ext: {
    identifier = {
      id = ext.vscodeExtUniqueId;
    };
    version = ext.version or "1.0.0";
    location = {
      "$mid" = 1;
      path = "${config.home.homeDirectory}/.vscode-server/extensions/${ext.vscodeExtUniqueId}";
      scheme = "file";
    };
    relativeLocation = ext.vscodeExtUniqueId;
    metadata = {
      id = ext.vscodeExtUniqueId;
      publisherDisplayName = ext.vscodeExtPublisher or "Unknown";
      publisherId = ext.vscodeExtPublisher or "unknown";
      isPreReleaseVersion = false;
    };
  };

  nixExtensionsJson = map mkExtensionEntry commonExtensions;

  # 공통 설정
  userSettings = {
    # Editor
    "editor.fontFamily" = "'JetBrainsMono Nerd Font', 'Sarasa Gothic J', 'Sarasa Gothic K', monospace";
    "editor.fontSize" = 14;
    "editor.formatOnPaste" = true;
    "editor.formatOnSave" = true;
    "editor.inlineSuggest.enabled" = true;
    "editor.tabSize" = 2;

    # Explorer
    "explorer.confirmDelete" = false;
    "explorer.confirmDragAndDrop" = false;
    "explorer.confirmPasteNative" = false;

    # Files
    "files.autoSave" = "afterDelay";
    "files.exclude" = {
      ".pytest_cache" = true;
      ".serena" = true;
      "**/__pycache__" = true;
      "**/.direnv" = true;
      "**/.idea" = true;
      "**/dist" = true;
      "**/node_modules" = true;
    };
    "files.insertFinalNewline" = true;

    # Workbench
    "workbench.colorTheme" = "One Half Light";
    "workbench.iconTheme" = "vscode-icons";
    "workbench.list.typeNavigationMode" = "filter";

    # Window
    "window.openFoldersInNewWindow" = "on";

    # Debug
    "debug.internalConsoleOptions" = "openOnSessionStart";

    # JavaScript
    "javascript.updateImportsOnFileMove.enabled" = "always";

    # Vim
    "vim.easymotion" = true;
    "vim.enableNeovim" = true;
    "vim.useCtrlKeys" = true;

    # Python
    "python.languageServer" = "None";
    "python.analysis.typeCheckingMode" = "standard";
    "python.venvPath" = "\${workspaceFolder}/.venv";
    "python.venvFolders" = [
      ".venv"
      "venv"
      ".env"
      "env"
      ".virtualenvs"
    ];
    "[python]" = {
      "editor.codeActionsOnSave" = {
        "source.fixAll" = "explicit";
        "source.organizeImports" = "explicit";
      };
      "editor.formatOnSave" = true;
      "editor.tabSize" = 4;
    };

    # Ruff
    "ruff.configuration" = "";
    "ruff.configurationPreference" = "filesystemFirst";

    # Dart
    "[dart]" = {
      "editor.formatOnSave" = true;
      "editor.formatOnType" = true;
      "editor.rulers" = [ 80 ];
      "editor.selectionHighlight" = false;
      "editor.tabCompletion" = "onlySnippets";
      "editor.wordBasedSuggestions" = "off";
    };

    # GitHub Copilot
    "github.copilot.advanced" = {
      "useLanguageServer" = true;
    };
    "github.copilot.enable" = {
      "*" = true;
      "markdown" = true;
      "plaintext" = false;
      "scminput" = false;
    };
    "github.copilot.nextEditSuggestions.enabled" = true;

    # Kubernetes
    "vs-kubernetes" = {
      "vs-kubernetes.crd-code-completion" = "enabled";
    };

    # Docker Compose
    "[dockercompose]" = {
      "editor.autoIndent" = "advanced";
      "editor.defaultFormatter" = "redhat.vscode-yaml";
      "editor.insertSpaces" = true;
      "editor.quickSuggestions" = {
        "comments" = false;
        "other" = true;
        "strings" = true;
      };
      "editor.tabSize" = 2;
    };

    # GitHub Actions
    "[github-actions-workflow]" = {
      "editor.defaultFormatter" = "redhat.vscode-yaml";
    };

    # Claude Code
    "claudeCode.preferredLocation" = "panel";
  };
in
{
  options.modules.vscode = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Enable VSCode module (currently disabled, use Zed instead)";
    };
  };

  config = lib.mkIf cfg.enable (
    lib.mkMerge [
      # macOS/Linux (non-WSL): 전체 VSCode 설정
      (lib.mkIf (!isWSL) {
        programs.vscode = {
          enable = true;
          package = pkgs.vscode;
          mutableExtensionsDir = true;

          profiles.default = {
            extensions = commonExtensions;
            inherit userSettings;
          };
        };
      })

      # WSL: 확장과 설정을 심볼릭 링크로 설치 (VSCode는 Windows에서 실행)
      (lib.mkIf isWSL {
        home.file = wslExtensionLinks // {
          ".vscode-server/data/Machine/settings.json".source = wslSettingsFile;
        };

        # extensions.json에 Nix 확장들을 병합
        home.activation.vscodeExtensionsJson =
          let
            nixExtensionsFile = pkgs.writeText "nix-extensions.json" (builtins.toJSON nixExtensionsJson);
            extensionsJsonPath = "${config.home.homeDirectory}/.vscode-server/extensions/extensions.json";
          in
          lib.hm.dag.entryAfter [ "writeBoundary" ] ''
            if [ -f "${extensionsJsonPath}" ]; then
              # 기존 extensions.json과 Nix 확장들을 병합 (Nix 확장 ID로 중복 제거)
              ${pkgs.jq}/bin/jq -s '
                (.[0] | map({key: .identifier.id, value: .}) | from_entries) as $existing |
                (.[1] | map({key: .identifier.id, value: .}) | from_entries) as $nix |
                ($existing + $nix) | to_entries | map(.value)
              ' "${extensionsJsonPath}" "${nixExtensionsFile}" > "${extensionsJsonPath}.tmp"
              mv "${extensionsJsonPath}.tmp" "${extensionsJsonPath}"
            else
              # extensions.json이 없으면 새로 생성
              mkdir -p "$(dirname "${extensionsJsonPath}")"
              cp "${nixExtensionsFile}" "${extensionsJsonPath}"
            fi
          '';
      })
    ]
  );
}
