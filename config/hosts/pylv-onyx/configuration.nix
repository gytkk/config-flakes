{
  config,
  inputs,
  pkgs,
  ...
}:
{
  imports = [
    inputs.disko.nixosModules.disko
    inputs.niri.nixosModules.niri
    inputs.dms.nixosModules.dank-material-shell
    ./hardware-configuration.nix
    ../../modules/openclaw
    ../../modules/nixos
  ];

  modules.openclaw = {
    enable = true;
    lanInterface = "wlo1";
  };

  boot.loader.systemd-boot.enable = true;
  boot.loader.systemd-boot.graceful = true;
  boot.loader.efi.canTouchEfiVariables = false;
  boot.kernelPackages = pkgs.linuxPackages_6_12;

  hardware.enableRedistributableFirmware = true;
  hardware.graphics.enable = true;

  # NVIDIA GTX 1650 Mobile (Turing) — offload 모드: 평소 Intel iGPU, 필요 시 nvidia-offload 명령으로 사용
  hardware.nvidia = {
    modesetting.enable = true;
    open = false;
    nvidiaSettings = true;
    package = config.boot.kernelPackages.nvidiaPackages.stable;
    prime = {
      offload = {
        enable = true;
        enableOffloadCmd = true;
      };
      intelBusId = "PCI:0:2:0";
      nvidiaBusId = "PCI:87:0:0";
    };
  };

  networking.networkmanager.enable = true;
  networking.hostName = "pylv-onyx";

  users.users.gytkk.openssh.authorizedKeys.keys = [
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIPSLJRqb27foz3LyICtfk8A+VyyjXdkjQOp6rG+MX28E u0_a391@localhost"
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIIWR6uTz4NEgOW8T/pI7Czk3WGpXIyobVqNkf4YrGUOd gytkk-windows"
  ];

  # Keep direct `nixos-rebuild switch` from attempting the dbus -> broker live migration.
  services.dbus.implementation = "dbus";

  # niri compositor
  programs.niri.enable = true;
  # Use niri-flake's pinned package set for its required libdisplay-info version.
  programs.niri.package = inputs.niri.packages.${pkgs.stdenv.hostPlatform.system}.niri-stable;

  # DankMaterialShell
  programs.dank-material-shell.enable = true;

  services.displayManager.dms-greeter = {
    enable = true;
    compositor.name = "niri";
  };

  services.libinput = {
    enable = true;
    touchpad.naturalScrolling = true;
  };

  services.pipewire = {
    enable = true;
    alsa.enable = true;
    alsa.support32Bit = true;
    pulse.enable = true;
  };
  security.rtkit.enable = true;

  # 노트북 덮개를 닫아도 suspend하지 않음 (서버 용도)
  services.logind.settings.Login = {
    HandleLidSwitch = "ignore";
    HandleLidSwitchExternalPower = "ignore";
    HandleLidSwitchDocked = "ignore";
  };

  # Caps Lock → Left Ctrl (커널 레벨, 모든 DE/TTY에서 동작)
  services.keyd = {
    enable = true;
    keyboards.default = {
      ids = [ "*" ];
      settings.main.capslock = "leftcontrol";
    };
  };

  # Host-specific packages
  environment.systemPackages = with pkgs; [
    # X11 앱 호환 (Electron 등)
    xwayland-satellite-stable
  ];

  # Wayland Electron support and headless user-systemd defaults for login shells.
  environment.sessionVariables = {
    NIXOS_OZONE_WL = "1";
  };

  # CJK fallback 글꼴
  fonts.fontconfig.defaultFonts = {
    sansSerif = [
      "Pretendard"
      "Sarasa Gothic K"
    ];
    serif = [ "Sarasa Gothic K" ];
    monospace = [ "Sarasa Mono K" ];
  };

  # 한글 입력기
  i18n.inputMethod = {
    enable = true;
    type = "fcitx5";
    fcitx5.addons = with pkgs; [
      fcitx5-hangul
      fcitx5-gtk
    ];
  };

  system.stateVersion = "25.11";
}
