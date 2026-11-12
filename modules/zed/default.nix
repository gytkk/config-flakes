{
  config,
  lib,
  pkgs,
  username,
  flakeDirectory,
  themeExports,
  isWSL ? false,
  ...
}:

let
  cfg = config.modules.zed;

  mkSymlink = path: config.lib.file.mkOutOfStoreSymlink "${flakeDirectory}/modules/zed/${path}";
  zedThemeExportsPath = themeExports.mutableDir "zed";
  zedThemeExports = config.lib.file.mkOutOfStoreSymlink zedThemeExportsPath;
  zedExtensions = pkgs.zed-extensions // {
    # proto pulls in a large Rust workspace; unrestricted Cargo jobs can OOM on pylv-onyx.
    proto = pkgs.zed-extensions.proto.overrideAttrs (_: {
      CARGO_BUILD_JOBS = "1";
    });
  };

  # Nix로 관리할 확장 목록 (pkgs.zed-extensions에서 가져옴)
  nixExtensions = with zedExtensions; [
    docker-compose
    dockerfile
    git-firefly
    make

    # Languages
    java
    scala
    sql
    nix
    proto
    toml
    html
    terraform
  ];

  # 모든 확장을 하나의 디렉토리로 병합
  extensionsDir = pkgs.runCommand "zed-extensions-merged" { } ''
    mkdir -p $out
    ${lib.concatMapStringsSep "\n" (ext: ''
      if [ -d "${ext}/share/zed/extensions" ]; then
        for dir in ${ext}/share/zed/extensions/*; do
          if [ -d "$dir" ]; then
            cp -rL "$dir" $out/
          fi
        done
      fi
    '') nixExtensions}
  '';

  # Zed config/data 경로 (플랫폼별)
  # Zed 0.222+ 부터 macOS에서도 config 경로가 ~/.config/zed 로 변경됨
  zedConfigPath = ".config/zed";

  zedDataPath =
    if pkgs.stdenv.hostPlatform.isDarwin then "Library/Application Support/Zed" else ".local/share/zed";

  # WSL: Windows Zed 경로
  windowsZedConfigPath = "/mnt/c/Users/${username}/AppData/Roaming/Zed";
  windowsZedDataPath = "/mnt/c/Users/${username}/AppData/Local/Zed";

  # WSL activation script input files
  settingsFile = "${flakeDirectory}/modules/zed/files/settings.json";
  keymapFile = pkgs.writeText "zed-keymap.json" (builtins.readFile ./files/keymap.json);

  # WSL: Windows Zed에 설정, 테마, 확장 배포 스크립트
  wslActivationScript = ''
    deploy_zed_config() (
      set -e
      config_path=${lib.escapeShellArg windowsZedConfigPath}
      data_path=${lib.escapeShellArg windowsZedDataPath}
      settings_file=${lib.escapeShellArg settingsFile}
      keymap_file=${lib.escapeShellArg keymapFile}
      themes_path=${lib.escapeShellArg zedThemeExportsPath}
      extensions_path=${lib.escapeShellArg (toString extensionsDir)}
      mkdir -p "$config_path/themes"
      mkdir -p "$data_path/extensions/installed"

      stage=$(mktemp -d "$config_path/.home-manager.XXXXXX")
      extension_stage=""
      cleanup_zed_stage() {
        rm -rf "$stage"
        if [ -n "$extension_stage" ]; then
          if [ -e "$extension_stage/previous" ] || [ -L "$extension_stage/previous" ]; then
            if [ -e "$target" ] || [ -L "$target" ] || ! mv -T "$extension_stage/previous" "$target"; then
              echo "Preserved previous Zed extension at $extension_stage/previous" >&2
              return
            fi
          fi
          rm -rf "$extension_stage"
        fi
      }
      trap cleanup_zed_stage EXIT
      mkdir "$stage/themes"
      cp "$settings_file" "$stage/settings.json"
      cmp "$settings_file" "$stage/settings.json"
      cp "$keymap_file" "$stage/keymap.json"
      cmp "$keymap_file" "$stage/keymap.json"

      for theme in "$themes_path"/*.json; do
        [ -f "$theme" ] || continue
        cp "$theme" "$stage/themes/$(basename "$theme")"
        cmp "$theme" "$stage/themes/$(basename "$theme")"
      done

      # Stage every config before replacing files; keep the latest pre-replacement bytes.
      for file in "$stage"/*.json "$stage/themes"/*.json; do
        [ -f "$file" ] || continue
        target="$config_path/''${file#"$stage/"}"
        if cmp -s "$file" "$target"; then
          continue
        fi
        if [ -f "$target" ]; then
          cp -p "$target" "$stage/backup"
          mv -T "$stage/backup" "$target.bak"
          chmod --reference="$target" "$file"
        fi
        mv -T "$file" "$target"
      done

      # 확장 복사 (Nix store → Windows 경로)
      for ext in "$extensions_path"/*; do
        [ -d "$ext" ] || continue
        ext_name=$(basename "$ext")
        target="$data_path/extensions/installed/$ext_name"
        extension_stage=$(mktemp -d "$data_path/extensions/installed/.home-manager.XXXXXX")
        cp -rL "$ext" "$extension_stage/new"
        if [ -e "$target" ] || [ -L "$target" ]; then
          mv -T "$target" "$extension_stage/previous"
        fi
        mv -T "$extension_stage/new" "$target"
        rm -rf "$extension_stage"
        extension_stage=""
      done

      echo "Zed config deployed to Windows:"
      echo "  - Settings: $config_path"
      echo "  - Keymap: $config_path"
      echo "  - Extensions: $data_path/extensions/installed"
    )
    run deploy_zed_config
  '';
in
{
  options.modules.zed = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Enable Zed editor module";
    };
  };

  config = lib.mkIf cfg.enable (
    lib.mkMerge [
      # macOS/Linux (non-WSL): 설정 파일을 repo에 직접 symlink
      (lib.mkIf (!isWSL) {
        # 설정 파일은 앱에서 수정하면 repo에 바로 반영되도록 직접 symlink한다.
        home.file."${zedConfigPath}/settings.json".source = mkSymlink "files/settings.json";
        home.file."${zedConfigPath}/keymap.json".source = mkSymlink "files/keymap.json";

        # 생성된 테마 전체 → repo export 디렉토리로 직접 symlink (mutable)
        home.file."${zedConfigPath}/themes".source = zedThemeExports;

        # Nix로 확장 관리 (읽기 전용 — Nix 패키지 기반)
        home.file."${zedDataPath}/extensions/installed" = {
          recursive = true;
          force = true;
          source = extensionsDir;
        };
      })

      # WSL: Windows Zed에 설정, 테마, 확장 배포
      (lib.mkIf isWSL {
        home.activation.zedWindowsConfig = lib.hm.dag.entryAfter [ "writeBoundary" ] wslActivationScript;
      })
    ]
  );
}
