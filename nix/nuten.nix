{ lib, craneLib }:
let
  members = [
    ../crates
    ../tasks/oracle
  ];
  cargoFiles = lib.fileset.unions (
    [
      ../Cargo.toml
      ../Cargo.lock
      ../.cargo
    ]
    ++ map craneLib.fileset.commonCargoSources members
  );
  testFiles = lib.fileset.unions (
    map (lib.fileset.fileFilter (file: file.hasExt "nu" || file.hasExt "snap")) members
  );
  src = lib.fileset.toSource {
    root = ../.;
    fileset = cargoFiles;
  };
  testSrc = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.union cargoFiles testFiles;
  };

  commonArgs = {
    inherit src;
    strictDeps = true;
  };
  cargoArtifacts = craneLib.buildDepsOnly commonArgs;

  package = craneLib.buildPackage (
    commonArgs
    // {
      inherit cargoArtifacts;
      doCheck = false; # Tested by the nextest check
      cargoExtraArgs = "--locked --package nuten";
      meta = {
        description = "Formatter for Nushell";
        homepage = "https://github.com/mst-mkt/nuten";
        license = lib.licenses.mit;
        mainProgram = "nuten";
      };
    }
  );
in
{
  inherit package;

  checks = {
    build = package;
    clippy = craneLib.cargoClippy (
      commonArgs
      // {
        inherit cargoArtifacts;
        cargoClippyExtraArgs = "--all-targets -- --deny warnings";
      }
    );
    nextest = craneLib.cargoNextest (
      commonArgs
      // {
        inherit cargoArtifacts;
        src = testSrc;
        cargoNextestExtraArgs = "--no-tests=pass";
        env = {
          NUTEN_TEST_SKIP_NU = "1"; # No nu in the sandbox
          NUTEN_TEST_SKIP_CORPUS = "1"; # No corpus in the sandbox
        };
      }
    );
    doc = craneLib.cargoDoc (
      commonArgs
      // {
        inherit cargoArtifacts;
        env.RUSTDOCFLAGS = "--deny warnings";
      }
    );
  };
}
