{
  pkgs,
  inputs,
  perSystem,
  system,
  ...
}:
let
  rustPkgs = import inputs.nixpkgs {
    inherit system;
    overlays = [ inputs.rust-overlay.overlays.default ];
  };
  rust = rustPkgs.rust-bin.stable.latest.default.override {
    extensions = [
      "rust-src"
      "rust-analyzer"
    ];
  };
in
pkgs.mkShell {
  inputsFrom = [ perSystem.self.rubar ];
  packages = with pkgs; [
    rust
    tombi
    nixd
    nixfmt
    typos-lsp
  ];
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath perSystem.self.rubar.buildInputs;
}
