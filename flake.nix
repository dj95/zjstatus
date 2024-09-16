{
  description = "A configurable and themable statusbar for zellij.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane = {
      url = "github:ipetkov/crane";
    };

    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, crane, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        rustWithWasiTarget = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-std" "rust-analyzer" ];
          targets = [ "wasm32-wasip1" ];
        };

        # NB: we don't need to overlay our custom toolchain for the *entire*
        # pkgs (which would require rebuidling anything else which uses rust).
        # Instead, we just want to update the scope that crane will use by appending
        # our specific toolchain there.
        craneLib = (crane.mkLib pkgs).overrideToolchain rustWithWasiTarget;

        zjstatus = craneLib.buildPackage {
          src = craneLib.cleanCargoSource (craneLib.path ./.);

          cargoExtraArgs = "--target wasm32-wasip1";

          # Tests currently need to be run via `cargo wasi` which
          # isn't packaged in nixpkgs yet...
          doCheck = false;
          doNotSign = true;

          buildInputs = [
            # Add additional build inputs here
            pkgs.libiconv
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            # Additional darwin specific inputs can be set here
          ];
        };
      in
      {
        checks = {
          inherit zjstatus;
        };

        packages.default = zjstatus;

        devShells.default = craneLib.devShell {
          # Inherit inputs from checks.
          checks = self.checks.${system};

          # Extra inputs can be added here; cargo and rustc are provided by default
          # from the toolchain that was specified earlier.
          packages = with pkgs; [
            just
            rustWithWasiTarget
            cargo-audit
            cargo-component
            cargo-edit
            cargo-nextest
            cargo-watch
            clippy
            libiconv
            wasmtime
          ];
        };
      }
    );
}
