{
  inputs = {
    nixpkgs = {
      url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane = {
      url = "github:ipetkov/crane";
    };
  };

  outputs =
    inputs:
    let
      inherit (inputs.nixpkgs) lib;

      forAllSystems = lib.genAttrs [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];

      nuten = forAllSystems (
        system:
        let
          craneLib = (inputs.crane.mkLib inputs.nixpkgs.legacyPackages.${system}).overrideToolchain (
            _:
            inputs.fenix.packages.${system}.stable.withComponents [
              "cargo"
              "rustc"
              "clippy"
            ]
          );
          sourceOf =
            fileset:
            lib.fileset.toSource {
              root = ./.;
              inherit fileset;
            };
          cargoSources = craneLib.fileset.commonCargoSources ./.;
          testData = lib.fileset.fileFilter (file: file.hasExt "nu" || file.hasExt "snap") ./crates;
          commonArgs = {
            src = sourceOf cargoSources;
            strictDeps = true;
          };
          args = commonArgs // {
            cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          };
        in
        {
          package = craneLib.buildPackage (args // { doCheck = false; });
          clippy = craneLib.cargoClippy (
            args // { cargoClippyExtraArgs = "--all-targets -- --deny warnings"; }
          );
          nextest = craneLib.cargoNextest (
            args
            // {
              src = sourceOf (lib.fileset.union cargoSources testData);
              cargoNextestExtraArgs = "--no-tests=pass";
            }
          );
          doc = craneLib.cargoDoc (args // { env.RUSTDOCFLAGS = "--deny warnings"; });
        }
      );
    in
    {
      packages = forAllSystems (system: {
        default = nuten.${system}.package;
      });

      checks = forAllSystems (system: {
        build = nuten.${system}.package;
        inherit (nuten.${system}) clippy nextest doc;
      });
    };
}
