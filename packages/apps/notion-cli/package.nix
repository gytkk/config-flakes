{
  lib,
  mkTarballCli,
}:

let
  version = "0.23.31";
in
mkTarballCli {
  pname = "notion-cli";
  inherit version;
  platforms = {
    x86_64-linux = {
      target = "x86_64-unknown-linux-musl";
      hash = "sha256-BRWPqQml9ELfFY8qJMEs5LGrufmQRsOu37RxswXRjGg=";
    };
    aarch64-linux = {
      target = "aarch64-unknown-linux-musl";
      hash = "sha256-C4M7d2v9T7IJ+DjwKVaVD5HCgt61TgNbdmSr3AW2dUQ=";
    };
    x86_64-darwin = {
      target = "x86_64-apple-darwin";
      hash = "sha256-Fd17VHVWi0Iww+ZCM/ykpJT510jEBAPFWiiAYEvYFFg=";
    };
    aarch64-darwin = {
      target = "aarch64-apple-darwin";
      hash = "sha256-uDMx9ILEPRCeX1xIuMXzw79joG8GhEQ3GTgB6PI+8dU=";
    };
  };

  url = platform: "https://ntn.dev/releases/v${version}/ntn-${platform.target}.tar.gz";
  sourceRoot = platform: "ntn-${platform.target}";

  extraInstall = ''
    install -Dm644 README.md "$out/share/doc/notion-cli/README.md"
    install -Dm644 LICENSE.md "$out/share/licenses/notion-cli/LICENSE.md"
  '';

  meta = {
    description = "Official Notion CLI for Workers and public API operations";
    homepage = "https://github.com/makenotion/cli";
    license = lib.licenses.mit;
    mainProgram = "ntn";
  };
}
