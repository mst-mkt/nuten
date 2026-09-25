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

      nutenOf =
        pkgs:
        let
          craneLib = (inputs.crane.mkLib pkgs).overrideToolchain (
            p:
            inputs.fenix.packages.${p.stdenv.hostPlatform.system}.stable.withComponents [
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
          package = craneLib.buildPackage (
            args
            // {
              doCheck = false;
              cargoExtraArgs = "--locked --package nuten";
              meta = {
                description = "Formatter for Nushell";
                homepage = "https://github.com/mst-mkt/nuten";
                license = lib.licenses.mit;
                mainProgram = "nuten";
              };
            }
          );
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
        };

      nuten = forAllSystems (system: nutenOf inputs.nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (system: {
        default = nuten.${system}.package;
      });

      overlays.default = final: _prev: { nuten = (nutenOf final).package; };

      treefmtModule =
        {
          lib,
          pkgs,
          config,
          mkFormatterModule,
          ...
        }:
        let
          cfg = config.programs.nuten;
        in
        {
          imports = [
            (mkFormatterModule {
              name = "nuten";
              args = [ "format" ];
              includes = [ "*.nu" ];
            })
          ];

          options.programs.nuten.settings = lib.mkOption {
            type = lib.types.submodule { freeformType = lib.types.json; };
            default = { };
            example = {
              line_width = 80;
            };
            description = ''
              Configuration to generate nuten.json with.
              When not empty, it is passed with `--config`, and config files in the project are ignored.
            '';
          };

          config = lib.mkMerge [
            { programs.nuten.package = lib.mkDefault (nutenOf pkgs).package; }
            (lib.mkIf (cfg.enable && cfg.settings != { }) {
              settings.formatter.nuten.options = [
                "--config"
                (toString (pkgs.writeText "nuten.json" (builtins.toJSON cfg.settings)))
              ];
            })
          ];
        };

      checks = forAllSystems (system: {
        build = nuten.${system}.package;
        inherit (nuten.${system}) clippy nextest doc;
      });
    };
}
