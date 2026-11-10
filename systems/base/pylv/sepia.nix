{ inputs, pkgs, ... }:
{
  # pylv-sepia 환경 전용 설정

  home.packages = [
    inputs.gws.packages.${pkgs.system}.default
    pkgs.google-cloud-sdk
  ];

  # 서버 환경에서 불필요한 GUI 모듈 비활성화
  modules.zed.enable = false;
}
