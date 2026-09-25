{
  inputs = {
    nixpkgs = {
      url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nu_scripts = {
      url = "github:nushell/nu_scripts";
      flake = false;
    };
    nixpkgs-nushell = {
      # nushell 0.115
      url = "github:NixOS/nixpkgs/aff8a0b28396750446e5537a96461bc4facdb287";
    };
  };

  outputs =
    inputs:
    let
      forAllSystems = inputs.nixpkgs.lib.genAttrs [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];

      rustToolchain = forAllSystems (
        system:
        with inputs.fenix.packages.${system};
        combine [
          (stable.withComponents [
            "cargo"
            "rustc"
            "clippy"
            "rust-src"
            "rust-analyzer"
          ])
          latest.rustfmt
        ]
      );

      treefmtEval = forAllSystems (
        system:
        inputs.treefmt-nix.lib.evalModule inputs.nixpkgs.legacyPackages.${system} {
          projectRootFile = "Cargo.toml";

          programs = {
            nixfmt = {
              enable = true;
              strict = true;
            };
            deadnix = {
              enable = true;
            };
            statix = {
              enable = true;
            };
            rustfmt = {
              enable = true;
              package = rustToolchain.${system};
            };
            taplo = {
              enable = true;
            };
            oxfmt = {
              enable = true;
            };
          };
        }
      );

      preCommit = forAllSystems (
        system:
        let
          pkgs = inputs.nixpkgs.legacyPackages.${system};

          commitTypes = [
            "feat"
            "add"
            "fix"
            "refactor"
            "perf"
            "style"
            "test"
            "docs"
            "ci"
            "chore"
          ];
          commitScopes = [
            "cli"
            "syntax"
            "parser"
            "doc"
            "format"
            "config"
            "nix"
            "oracle"
          ];
        in
        inputs.git-hooks.lib.${system}.run {
          src = ./..;
          hooks = {
            treefmt = {
              enable = true;
              package = treefmtEval.${system}.config.build.wrapper;
            };
            clippy = {
              enable = true;
              packageOverrides = {
                cargo = rustToolchain.${system};
                clippy = rustToolchain.${system};
              };
              settings = {
                denyWarnings = true;
                extraArgs = "--all-targets";
              };
            };
            typos = {
              enable = true;
            };
            check-merge-conflicts = {
              enable = true;
            };
            convco = {
              enable = true;
              settings.configPath = (pkgs.formats.yaml { }).generate "convco.yaml" {
                types = map (type: {
                  inherit type;
                  section = type;
                }) commitTypes;
                scopeRegex = "^(${pkgs.lib.concatStringsSep "|" commitScopes})$";
              };
            };
          };
        }
      );
    in
    {
      devShells = forAllSystems (
        system:
        let
          pkgs = inputs.nixpkgs.legacyPackages.${system};
          nushell = inputs.nixpkgs-nushell.legacyPackages.${system}.nushell;
        in
        {
          default = pkgs.mkShell {
            packages = [
              nushell
              rustToolchain.${system}
              pkgs.cargo-insta
              pkgs.cargo-nextest
              treefmtEval.${system}.config.build.wrapper
            ];
            env = {
              NUTEN_CORPUS = pkgs.runCommand "nuten-corpus" { } ''
                mkdir $out
                cp -r ${inputs.nu_scripts} $out/nu_scripts
                cp -r ${nushell.src}/crates/nu-std $out/nu-std
              '';
            };
            inherit (preCommit.${system}) shellHook;
          };

          fuzz = pkgs.mkShell {
            packages = [
              (inputs.fenix.packages.${system}.latest.withComponents [
                "cargo"
                "rustc"
              ])
              pkgs.cargo-fuzz
            ];
            env = {
              LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [ pkgs.stdenv.cc.cc.lib ];
            };
          };
        }
      );

      checks = forAllSystems (system: {
        treefmt = treefmtEval.${system}.config.build.check (inputs.nixpkgs.lib.cleanSource ./..);
      });

      formatter = forAllSystems (system: treefmtEval.${system}.config.build.wrapper);
    };
}
