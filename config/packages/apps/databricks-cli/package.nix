{
  lib,
  mkTarballCli,
}:

let
  version = "1.19.0";
in
mkTarballCli {
  pname = "databricks-cli";
  inherit version;
  platforms = {
    x86_64-linux = {
      name = "linux_amd64";
      hash = "sha256-vd+H3z1ythOE2nJLQPGKtIOpivdCdIJgCXp7uBVLBw4=";
    };
    aarch64-linux = {
      name = "linux_arm64";
      hash = "sha256-KZfMvaVwkO5IvZKC1cs6W+94Al22T9FXP718hyaFYIY=";
    };
    x86_64-darwin = {
      name = "darwin_amd64";
      hash = "sha256-0ppdwep9RNDYkv1L6sBdItqpMpgspr+YoG1SyV9e1M0=";
    };
    aarch64-darwin = {
      name = "darwin_arm64";
      hash = "sha256-uExqjGI1eDCKckzdcYG5KwgWwlMVZPFvjedVmyo59R8=";
    };
  };

  url =
    platform:
    "https://github.com/databricks/cli/releases/download/v${version}/databricks_cli_${version}_${platform.name}.tar.gz";

  meta = {
    description = "Databricks CLI for the Databricks platform (Go implementation)";
    homepage = "https://github.com/databricks/cli";
    license = lib.licenses.asl20;
    mainProgram = "databricks";
  };
}
