{ lib, pkgs, ... }:
{
  # Enable flakes
  nix.settings = {
    experimental-features = [
      "nix-command"
      "flakes"
    ];
    min-free = lib.mkDefault (20 * 1024 * 1024 * 1024);
    max-free = lib.mkDefault (80 * 1024 * 1024 * 1024);
  };

  # Generation retention and garbage collection live in ./nix-gc.nix.
  nix.optimise = {
    automatic = lib.mkDefault true;
    dates = lib.mkDefault [ "Sun 05:00" ];
  };

  # Keep the current D-Bus daemon until we do an explicit broker migration.
  services.dbus.implementation = "dbus";

  # Enable zsh system-wide (configuration via Home Manager)
  programs.zsh.enable = true;

  # Enable nix-ld for running dynamically linked binaries (e.g., bun plugins)
  programs.nix-ld.enable = true;

  # /bin/bash shebang 호환성 (서드파티 스크립트용)
  system.activationScripts.binbash = ''
    ln -sfn ${pkgs.bash}/bin/bash /bin/bash
  '';

  # Locale - SSH 접속 시 클라이언트에서 전달되는 ko_KR.UTF-8 지원
  i18n.supportedLocales = [
    "en_US.UTF-8/UTF-8"
    "ko_KR.UTF-8/UTF-8"
  ];

  # Minimal system packages (most packages managed by Home Manager)
  environment.systemPackages = with pkgs; [
    curl
    dnsutils
    wget
    vim
    # Ghostty terminfo (SSH 접속 시 xterm-ghostty TERM 인식용)
    ghostty.terminfo
  ];
}
