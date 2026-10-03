{ craneLib }:
let
  src = craneLib.cleanCargoSource ../editors/zed;

  commonArgs = {
    inherit src;
    strictDeps = true;
  };
  wasmArgs = commonArgs // {
    CARGO_BUILD_TARGET = "wasm32-wasip2";
    doCheck = false; # Tested natively by zed-extension-test
  };
  wasmArtifacts = craneLib.buildDepsOnly wasmArgs;
  nativeArtifacts = craneLib.buildDepsOnly commonArgs;
in
{
  checks = {
    zed-extension = craneLib.buildPackage (wasmArgs // { cargoArtifacts = wasmArtifacts; });
    zed-extension-clippy = craneLib.cargoClippy (
      wasmArgs
      // {
        cargoArtifacts = wasmArtifacts;
        cargoClippyExtraArgs = "--all-targets -- --deny warnings";
      }
    );
    zed-extension-test = craneLib.cargoTest (commonArgs // { cargoArtifacts = nativeArtifacts; });
  };
}
