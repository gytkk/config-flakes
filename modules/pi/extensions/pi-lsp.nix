{ pkgs }:

pkgs.applyPatches {
  name = "pi-lsp-refactor-0.1.7";
  src = pkgs.fetchzip {
    url = "https://registry.npmjs.org/pi-lsp/-/pi-lsp-0.1.7.tgz";
    sha256 = "12fhprz6y7i5skrf136b6ip7jnz930ms18cm5gci00m025zyb64b";
  };
  patches = [ ../lsp-refactor.patch ];
  postPatch = ''
    cp -r ${../files/extensions/lsp-refactor} extensions/pi-lsp/refactor
    echo 'export { default } from "./pi-lsp/index";' > extensions/index.ts
  '';
}
