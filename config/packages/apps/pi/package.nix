{
  callPackage,
  importNpmLock,
  lib,
  stdenv,
}:

let
  version = "1.5.1";
  # Upstream 1.5.0 omits pi-schema from both workspace packing and install overrides.
  needsSchemaFix = version == "1.5.0";
  # The upstream package reads lockfiles and the model catalog during evaluation.
  source = builtins.fetchTarball {
    url = "https://github.com/earendil-works/pi/archive/refs/tags/v${version}.tar.gz";
    sha256 = "sha256-G3SRvVaxOlgMY8bxl+SLAlF2+cMFQ2dBOuPo+NPKn/I=";
  };
in
callPackage "${source}/nix/package.nix" {
  inherit source;
  importNpmLock = importNpmLock // {
    importNpmLock =
      args:
      importNpmLock (
        args
        // lib.optionalAttrs (needsSchemaFix && args ? packageSourceOverrides) {
          packageSourceOverrides = args.packageSourceOverrides // {
            "node_modules/@earendil-works/pi-schema" =
              builtins.dirOf args.packageSourceOverrides."node_modules/@earendil-works/pi-ai" + "/schema.tgz";
          };
        }
      );
  };
  # Patch the workspace build, not source metadata read during evaluation or the final install lock.
  stdenv = stdenv // {
    mkDerivation =
      attrs:
      stdenv.mkDerivation (
        attrs
        // lib.optionalAttrs (attrs.pname == "pi-workspace-packages") {
          patches = (attrs.patches or [ ]) ++ import ./patches.nix;
          postInstall =
            (attrs.postInstall or "")
            + lib.optionalString needsSchemaFix ''
              pack_package packages/schema schema
            '';
          postPatch = (attrs.postPatch or "") + ''
            cp ${./herdr-file-opener.ts} packages/coding-agent/src/utils/herdr-file-opener.ts
          '';
        }
      );
  };
}
