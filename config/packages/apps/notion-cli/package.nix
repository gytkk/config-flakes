{
  lib,
  mkTarballCli,
}:

let
  version = "0.23.26";
in
mkTarballCli {
  pname = "notion-cli";
  inherit version;
  platforms = {
    x86_64-linux = {
      target = "x86_64-unknown-linux-musl";
      hash = "sha256-xsZxhvUCl0Dv6pLLgrlcqmkJ2MxXZffUz112kw7EWUI=";
    };
    aarch64-linux = {
      target = "aarch64-unknown-linux-musl";
      hash = "sha256-ANM0ERXMkQPuPsQ6z6y4FNVLH7cSazTBPsxOqKIu+Ik=";
    };
    x86_64-darwin = {
      target = "x86_64-apple-darwin";
      hash = "sha256-iuYxpfnfBi0AAKJ728VRvIDjpJv0wz/vP/F503Lq4XE=";
    };
    aarch64-darwin = {
      target = "aarch64-apple-darwin";
      hash = "sha256-bczJzc+tmzj6WXdQl/+mw0wwbPufV06peHM3rwSX1fg=";
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
