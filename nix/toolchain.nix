{
  pkgs,
  crane,
  fenix,
}:
let
  craneLibWith = (crane.mkLib pkgs).overrideToolchain;

  fenixPackages = fenix.packages.${pkgs.stdenv.hostPlatform.system};
  rust = fenixPackages.stable.withComponents [
    "cargo"
    "rustc"
    "clippy"
  ];
  nodejs = pkgs.nodejs-slim_26;
in
{
  craneLib = craneLibWith rust;
  wasmCraneLib = craneLibWith (
    fenixPackages.combine [
      rust
      fenixPackages.targets.wasm32-wasip2.stable.rust-std # For Zed extensions
    ]
  );
  inherit nodejs;
  pnpm = pkgs.pnpm_11.override { nodejs-slim = nodejs; };
}
