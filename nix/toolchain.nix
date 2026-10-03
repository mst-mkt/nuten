{
  pkgs,
  crane,
  fenix,
}:
let
  fenixPackages = fenix.packages.${pkgs.stdenv.hostPlatform.system};
  rust = fenixPackages.stable.withComponents [
    "cargo"
    "rustc"
  ];
  nodejs = pkgs.nodejs-slim_26;
in
{
  craneLib = (crane.mkLib pkgs).overrideToolchain rust;
  inherit nodejs;
  pnpm = pkgs.pnpm_11.override { nodejs-slim = nodejs; };
}
