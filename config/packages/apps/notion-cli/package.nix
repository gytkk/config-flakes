{
  lib,
  mkTarballCli,
}:

let
  version = "0.23.27";
in
mkTarballCli {
  pname = "notion-cli";
  inherit version;
  platforms = {
    x86_64-linux = {
      target = "x86_64-unknown-linux-musl";
      hash = "sha256-yAWBKHDkHqDRRSSjJl6muCnpchR+rdiKieWEUMoQ4oc=";
    };
    aarch64-linux = {
      target = "aarch64-unknown-linux-musl";
      hash = "sha256-Wsz9AdPfYpw3Urer3PS9z+rIQS44Ulz3KGMKYT+SW4o=";
    };
    x86_64-darwin = {
      target = "x86_64-apple-darwin";
      hash = "sha256-rx+zLtIZgxgJyf7xMR6VlO6BrjjwUNSQNGH3WYVM6Y0=";
    };
    aarch64-darwin = {
      target = "aarch64-apple-darwin";
      hash = "sha256-hGLdhHxzCWmvNKLYo9kovbkxz77hkGycuOm2PHcU0Ek=";
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
