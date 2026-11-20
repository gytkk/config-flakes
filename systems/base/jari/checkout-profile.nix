# Install once as a root-owned import. Select committed main, not the working tree.
let
  getFlake =
    if builtins.hasAttr "getFlake" builtins then
      builtins.getAttr "getFlake" builtins
    else
      throw "The jari profile requires flakes to be enabled on the worker.";
in
(getFlake "git+file:///workspace/repos/gytkk/config-flakes?ref=main").nixosModules.devsisters-jari
