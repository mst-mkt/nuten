{ lib, craneLib }:
let
  members = [
    ../crates
    ../integrations/dprint
    ../npm
    ../tasks/oracle
  ];
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions (
      [
        ../Cargo.toml
        ../Cargo.lock
        ../.cargo
      ]
      ++ map craneLib.fileset.commonCargoSources members
    );
  };

  commonArgs = {
    inherit src;
    strictDeps = true;
  };
  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
in
craneLib.buildPackage (
  commonArgs
  // {
    inherit cargoArtifacts;
    doCheck = false; # The tests need nu and the corpus, so CI runs them on each system
    cargoExtraArgs = "--locked --package nuten";
    meta = {
      description = "Formatter for Nushell";
      homepage = "https://github.com/mst-mkt/nuten";
      license = lib.licenses.mit;
      mainProgram = "nuten";
    };
  }
)
