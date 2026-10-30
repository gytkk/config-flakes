{
  lib,
  mkTarballCli,
}:

let
  version = "0.23.25";
in
mkTarballCli {
  pname = "notion-cli";
  inherit version;
  platforms = {
    x86_64-linux = {
      target = "x86_64-unknown-linux-musl";
      hash = "sha256-mfr4GvQDVTtm0PBqRmumumsOJwj7a48erCz3rR1dtH8=";
    };
    aarch64-linux = {
      target = "aarch64-unknown-linux-musl";
      hash = "sha256-g3mVTNuCo8YIZqGJI9pfQ6M1imyMaYwIXMKQXnJVqSc=";
    };
    x86_64-darwin = {
      target = "x86_64-apple-darwin";
      hash = "sha256-7KabwL9vSibIr/hgjXea66kjPXeAHZYtFenY/EXBtbs=";
    };
    aarch64-darwin = {
      target = "aarch64-apple-darwin";
      hash = "sha256-UxlsNvmPv5T1aDfdWZeHK1G69SJoJMPL2M99wflm6lw=";
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
