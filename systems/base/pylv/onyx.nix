{
  inputs,
  lib,
  pkgs,
  hasSystemNiriConfig,
  ...
}:
{
  # pylv-onyx 데스크톱 환경 전용 설정

  imports = (lib.optional (!hasSystemNiriConfig) inputs.niri.homeModules.config) ++ [
    inputs.dms.homeModules.dank-material-shell
    inputs.dms.homeModules.niri
    ../../../modules/openclaw/home.nix
  ];

  modules.openclaw.enable = true;

  # DankMaterialShell
  programs.dank-material-shell = {
    enable = true;
    systemd.enable = true;
    enableSystemMonitoring = true;
    enableDynamicTheming = true;
    enableClipboardPaste = true;
    niri = {
      enableKeybinds = true;
      includes.enable = false;
    };
  };

  home.packages = [
    pkgs.alacritty
    pkgs.btop
    pkgs.freecad
    pkgs.kicad
    pkgs.walker
    pkgs.wezterm
    pkgs.obsidian
    pkgs.wl-clipboard
    pkgs.yazi
    pkgs.zed-editor
    pkgs.pretendard
    pkgs.moonlight-qt
  ];

  # Alt+Space로 walker 실행 (Spotlight 스타일)
  programs.niri.settings.binds = {
    "Alt+Space".action.spawn = "walker";
  };
}
