# nixos-generate-config가 생성하며 다시 실행하면 덮어쓸 수 있는 파일이다.
# 변경 사항은 /etc/nixos/configuration.nix에 작성한다.
{
  config,
  lib,
  pkgs,
  modulesPath,
  ...
}:

{
  imports = [
    (modulesPath + "/installer/scan/not-detected.nix")
  ];

  boot.initrd.availableKernelModules = [
    "xhci_pci"
    "ahci"
    "nvme"
  ];
  boot.initrd.kernelModules = [ "dm-snapshot" ];
  boot.kernelModules = [ "kvm-intel" ];
  boot.extraModulePackages = [ ];

  # 기본 scripted networking에서는 모든 인터페이스에 DHCP를 적용한다.
  # systemd-networkd에서는 인터페이스별 useDHCP 설정과 함께 사용하는 편이 좋다.
  networking.useDHCP = lib.mkDefault true;
  # networking.interfaces.enp3s0.useDHCP = lib.mkDefault true;

  nixpkgs.hostPlatform = lib.mkDefault "x86_64-linux";
  hardware.cpu.intel.updateMicrocode = lib.mkDefault config.hardware.enableRedistributableFirmware;
}
