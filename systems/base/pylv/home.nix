{
  config,
  lib,
  pkgs,
  username,
  homeDirectory,
  ...
}:

{
  # Import base configuration
  imports = [ ../default.nix ];

  # Pylv 특화 shell aliases (추후 필요시 추가)
  home.shellAliases = {
    # 추후 필요한 별칭들 추가 예정
  };

  # Pylv 특화 환경 변수 (추후 필요시 추가)
  home.sessionVariables = {
    # 추후 필요한 환경 변수들 추가 예정
    LESSCHARSET = "utf-8";
  };
}
