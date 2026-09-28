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
          cargoSources = lib.fileset.difference (craneLib.fileset.commonCargoSources ./.) (
            lib.fileset.unions [
              ./tasks/fuzz
              ./editors/zed
            ]
          );
          testData = lib.fileset.unions (
            map (lib.fileset.fileFilter (file: file.hasExt "nu" || file.hasExt "snap")) [
              ./crates
              ./tasks/oracle
            ]
          );
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
              env = {
                NUTEN_TEST_SKIP_NU = "1"; # No nu in the sandbox
                NUTEN_TEST_SKIP_CORPUS = "1"; # No corpus in the sandbox
              };
            }
          );
          doc = craneLib.cargoDoc (args // { env.RUSTDOCFLAGS = "--deny warnings"; });
        };

      vscodeExtensionOf =
        pkgs:
        let
          manifest = lib.importJSON ./editors/vscode/package.json;
          pnpm = pkgs.pnpm_11.override { nodejs-slim = pkgs.nodejs-slim_26; };
        in
        pkgs.vscode-utils.buildVscodeExtension (finalAttrs: {
          pname = "${manifest.publisher}-${manifest.name}";
          inherit (manifest) version;
          vscodeExtPublisher = manifest.publisher;
          vscodeExtName = manifest.name;
          vscodeExtUniqueId = "${manifest.publisher}.${manifest.name}";

          src = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              ./package.json
              ./pnpm-lock.yaml
              ./pnpm-workspace.yaml
              ./LICENSE
              ./assets/logo.png
              ./editors/vscode/package.json
              ./editors/vscode/icon.png
              ./editors/vscode/LICENSE
              (lib.fileset.difference ./editors/vscode/src ./editors/vscode/src/test)
            ];
          };
          sourceRoot = finalAttrs.src.name;

          pnpmWorkspaces = [ manifest.name ];
          pnpmInstallFlags = [ "--prod" ];
          pnpmDeps = pkgs.fetchPnpmDeps {
            inherit (finalAttrs)
              pname
              version
              src
              pnpmWorkspaces
              pnpmInstallFlags
              ;
            inherit pnpm;
            fetcherVersion = 4;
            hash = "sha256-AmuKnNoq0ajzctosyzZUjDL4KOjN9Q3Potx3dO2fhfw=";
          };

          nativeBuildInputs = [
            pnpm
            pkgs.pnpmConfigHook
            pkgs.esbuild
          ];

          buildPhase = ''
            runHook preBuild
            cd editors/vscode
            esbuild src/extension.ts --bundle --platform=node --format=cjs --external:vscode --minify --outfile=${manifest.main}
            runHook postBuild
          '';

          installPhase = ''
            runHook preInstall
            mkdir -p "$out/$installPrefix"
            cp --dereference --parents --target-directory="$out/$installPrefix" package.json ${lib.escapeShellArgs manifest.files}
            runHook postInstall
          '';

          meta = {
            inherit (manifest) description;
            homepage = "https://github.com/mst-mkt/nuten";
            license = lib.licenses.mit;
            sourceProvenance = [ lib.sourceTypes.fromSource ];
          };
        });

      zedExtensionChecksOf =
        pkgs:
        let
          craneLib = (inputs.crane.mkLib pkgs).overrideToolchain (
            p:
            with inputs.fenix.packages.${p.stdenv.hostPlatform.system};
            combine [
              (stable.withComponents [
                "cargo"
                "rustc"
                "clippy"
              ])
              targets.wasm32-wasip2.stable.rust-std
            ]
          );
          commonArgs = {
            src = craneLib.cleanCargoSource ./editors/zed;
            strictDeps = true;
          };
          wasmArgs = commonArgs // {
            CARGO_BUILD_TARGET = "wasm32-wasip2";
            doCheck = false;
          };
          wasm = wasmArgs // {
            cargoArtifacts = craneLib.buildDepsOnly wasmArgs;
          };
        in
        {
          zed-extension = craneLib.buildPackage wasm;
          zed-extension-clippy = craneLib.cargoClippy (
            wasm // { cargoClippyExtraArgs = "--all-targets -- --deny warnings"; }
          );
          zed-extension-test = craneLib.cargoTest (
            commonArgs // { cargoArtifacts = craneLib.buildDepsOnly commonArgs; }
          );
        };

      nuten = forAllSystems (system: nutenOf inputs.nixpkgs.legacyPackages.${system});
      vscodeExtension = forAllSystems (system: vscodeExtensionOf inputs.nixpkgs.legacyPackages.${system});
      zedExtensionChecks = forAllSystems (
        system: zedExtensionChecksOf inputs.nixpkgs.legacyPackages.${system}
      );
    in
    {
      packages = forAllSystems (system: {
        default = nuten.${system}.package;
        vscode-extension = vscodeExtension.${system};
      });

      overlays.default = final: prev: {
        nuten = (nutenOf final).package;
        vscode-extensions = prev.vscode-extensions // {
          mst-mkt = (prev.vscode-extensions.mst-mkt or { }) // {
            nuten-vscode = vscodeExtensionOf final;
          };
        };
      };

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
              includes = [
                "*.nu"
                "*.nuon"
              ];
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

      checks = forAllSystems (
        system:
        {
          build = nuten.${system}.package;
          inherit (nuten.${system}) clippy nextest doc;
          vscode-extension = vscodeExtension.${system};
        }
        // zedExtensionChecks.${system}
      );
    };
}
