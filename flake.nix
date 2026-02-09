{
  description = "A Rust development environment";

  inputs = {
    nixpkgs.url = "nixpkgs";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # 配置 Rust 工具链
        # 这里选择了 stable 版本，并添加了 rust-src (IDE跳转定义必须)
        rustToolchain = pkgs.rust-bin.nightly.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" "miri" "llvm-tools"];
        };

        # 如果你需要特定版本，可以使用:
        # rustToolchain = pkgs.rust-bin.stable."1.75.0".default.override { ... };
        # 或者 Nightly:
        # rustToolchain = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override { ... });

      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            pkg-config  # 编译依赖库常用
            openssl     # 许多 Rust 网络库需要
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            # macOS 特有的依赖
            libiconv
            darwin.apple_sdk.frameworks.Security
            darwin.apple_sdk.frameworks.SystemConfiguration
          ];

          # 设置环境变量
          # 这行对于 VS Code 等编辑器找到标准库源码至关重要
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";

          shellHook = ''
            echo "🦀 Rust DevShell activated!"
            echo "Rust version: $(rustc --version)"
          '';
        };
      }
    );
}
