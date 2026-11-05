{
  lib,
  mkTarballCli,
}:

let
  version = "0.23.28";
in
mkTarballCli {
  pname = "notion-cli";
  inherit version;
  platforms = {
    x86_64-linux = {
      target = "x86_64-unknown-linux-musl";
      hash = "sha256-y73AMRe3hsMeHJJE+I1TWL7y+QkW/s2cLgYDre8C+2Q=";
    };
    aarch64-linux = {
      target = "aarch64-unknown-linux-musl";
      hash = "sha256-xcsq26txS7wNzIOkAhb74xzXQt+X+Ry3KUrRUx91KTE=";
    };
    x86_64-darwin = {
      target = "x86_64-apple-darwin";
      hash = "sha256-Ed6euf0o4El9sNBenDWMQ8z8PZCVk65WV10yi7gPVHE=";
    };
    aarch64-darwin = {
      target = "aarch64-apple-darwin";
      hash = "sha256-l1XK+f6qqiYrANqE2U0FcaHn3VVMjFMZLAw5454MZkM=";
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
