{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = { nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustPlatform = with pkgs; makeRustPlatform {
          cargo = rust-bin.stable.latest.minimal;
          rustc = rust-bin.stable.latest.minimal;
        };
      in {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            (rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" "rust-analyzer" ];
            })
          ];
        };
        packages.default = rustPlatform.buildRustPackage {
          pname = "nxd";
          version = "0.3.1";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
        };
      }
    );
}
