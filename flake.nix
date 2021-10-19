{
  inputs = {
    nixpkgs.url      = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url  = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let 
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
      in
      {
        devShell = pkgs.mkShell {
          buildInputs = with pkgs; [
            pkg-config
            glib
            cairo
            pango
            atk
            gtk3
            gdk-pixbuf
            rust-analyzer
            (rust-bin.stable.latest.minimal.override {
              extensions = [
                "rust-src"
                "cargo"
                "rustc"
                "clippy"
              ];
            })
            rust-bin.nightly."2021-10-19".rustfmt
          ];
        };
      }
    );
}
