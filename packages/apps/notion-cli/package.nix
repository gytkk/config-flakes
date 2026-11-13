{
  lib,
  mkTarballCli,
}:

let
  version = "0.23.30";
in
mkTarballCli {
  pname = "notion-cli";
  inherit version;
  platforms = {
    x86_64-linux = {
      target = "x86_64-unknown-linux-musl";
      hash = "sha256-HggsUgLMwSVJ2xSDENaySx7T4TuOxQc+qqxrUaTp/ao=";
    };
    aarch64-linux = {
      target = "aarch64-unknown-linux-musl";
      hash = "sha256-PN7de9BggJ8iHyGnZRagMSRkWLyriJW+w6s4twMUf34=";
    };
    x86_64-darwin = {
      target = "x86_64-apple-darwin";
      hash = "sha256-xSj/LaJQW/SfdDS9jKuM2nLX7McbwtwI8NJvKEevIes=";
    };
    aarch64-darwin = {
      target = "aarch64-apple-darwin";
      hash = "sha256-7E3CdCfXHTu9QUmJsYl5SLgesJK5TenOtLbgb3UK8BY=";
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
