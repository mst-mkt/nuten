{ pkgs, inputs }:
let
  inherit (pkgs.stdenv.hostPlatform) system;

  rootToolchain = import ../toolchain.nix {
    inherit pkgs;
    inherit (inputs.nuten.inputs) crane;
    inherit (inputs) fenix;
  };
  fenixPackages = inputs.fenix.packages.${system};

  vp = inputs.nix-vite-plus.packages.${system}.vp.override { inherit (rootToolchain) nodejs; };
  vite-plus-local-version = (pkgs.lib.importJSON ../../package.json).devDependencies.vite-plus;
in
{
  rust = fenixPackages.combine [
    (fenixPackages.stable.withComponents [
      "cargo"
      "rustc"
      "clippy"
      "rust-src"
      "rust-analyzer"
    ])
    fenixPackages.targets.wasm32-wasip2.stable.rust-std # For Zed extensions
    fenixPackages.targets.wasm32-unknown-unknown.stable.rust-std # For wasm hosts of the formatter
    fenixPackages.latest.rustfmt # For unstable rustfmt options
  ];
  nightlyRust = fenixPackages.latest.withComponents [
    "cargo"
    "rustc"
  ]; # For cargo-fuzz

  nushell = inputs.nushell.packages.${system}.default.overrideAttrs { doCheck = false; };

  inherit (rootToolchain) nodejs pnpm;
  vite-plus =
    assert pkgs.lib.assertMsg (
      vp.version == vite-plus-local-version
    ) "vite-plus differs between nix/dev/flake.lock and package.json";
    vp;
  wasm-bindgen-cli = pkgs.wasm-bindgen-cli_0_2_127;
}
