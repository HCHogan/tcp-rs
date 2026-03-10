{
  description = "A Rust development environment";

  inputs = {
    nixpkgs.url = "nixpkgs";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.nightly.latest.default.override {
          extensions = ["rust-src" "rust-analyzer" "miri" "llvm-tools"];
        };
        # rustToolchain = pkgs.rust-bin.stable."1.75.0".default.override { ... };
        # rustToolchain = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override { ... });
      in {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs;
            [
              rustToolchain
              pkg-config
              openssl
              mold
              # sccache
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              # macOS 特有的依赖
              libiconv
              # darwin.apple_sdk.frameworks.Security
              # darwin.apple_sdk.frameworks.SystemConfiguration
            ];

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";

          # RUSTC_WRAPPER = "${pkgs.sccache}/bin/sccache";
          # SCCACHE_CACHE_SIZE = "10G";
          # SCCACHE_DIR = "/data/builds/sccache";

          shellHook = ''
            echo "Rust version: $(rustc --version)"
          '';
        };
      }
    );
}
