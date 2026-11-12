{
  lib,
  mkTarballCli,
}:

let
  version = "0.23.29";
in
mkTarballCli {
  pname = "notion-cli";
  inherit version;
  platforms = {
    x86_64-linux = {
      target = "x86_64-unknown-linux-musl";
      hash = "sha256-HTkR5JYGIUbZVtGOJJVi09yPh5fjEg/5LC+Vfg3uCPo=";
    };
    aarch64-linux = {
      target = "aarch64-unknown-linux-musl";
      hash = "sha256-HiADQmja6hIhtRHOdNghSGGRp4ucT5nYflnDveECUAE=";
    };
    x86_64-darwin = {
      target = "x86_64-apple-darwin";
      hash = "sha256-1M1c1BDnCIj4GQaEmJ1JBoDLU2ASDoYFNtmQ2TuQD+c=";
    };
    aarch64-darwin = {
      target = "aarch64-apple-darwin";
      hash = "sha256-+g7zPqnQhw4t568sS8nWMqaKmaqXGqFdXLr4nz6mseU=";
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
