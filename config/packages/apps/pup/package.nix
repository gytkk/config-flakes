{
  lib,
  mkTarballCli,
}:

let
  version = "1.4.0";
in
mkTarballCli {
  pname = "pup";
  inherit version;
  platforms = {
    x86_64-linux = {
      asset = "Linux_x86_64";
      hash = "sha256-Iaqd83pDU86+dGi7R3XPb2nHnhbcE/4cZDxf4upERR0=";
    };
    aarch64-linux = {
      asset = "Linux_arm64";
      hash = "sha256-Q4wlPHTbMnuE8tsykiek4Q89X1z77r1qrX10yaBmc0w=";
    };
    x86_64-darwin = {
      asset = "Darwin_x86_64";
      hash = "sha256-HrxX1tIESKeLAnxf1RJqqOE3DzwzXALHulQwlsY02M0=";
    };
    aarch64-darwin = {
      asset = "Darwin_arm64";
      hash = "sha256-L4fF5BHJGaq3IxI08w4Zuwg4Dzd/3nqS+r+P7Qjhjkc=";
    };
  };

  url =
    platform:
    "https://github.com/DataDog/pup/releases/download/v${version}/pup_${version}_${platform.asset}.tar.gz";

  extraInstall = ''
    install -Dm644 README.md "$out/share/doc/pup/README.md"
    install -Dm644 LICENSE "$out/share/licenses/pup/LICENSE"
  '';

  meta = {
    description = "Datadog CLI, a companion with 200+ commands across 33+ Datadog products";
    homepage = "https://github.com/DataDog/pup";
    license = lib.licenses.asl20;
    mainProgram = "pup";
  };
}
