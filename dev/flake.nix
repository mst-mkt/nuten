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
    nixpkgs-nushell = {
      # nushell 0.115
      url = "github:NixOS/nixpkgs/aff8a0b28396750446e5537a96461bc4facdb287";
    };
    nutest = {
      url = "github:vyadh/nutest/v1.2.0";
      flake = false;
    };

    # Corpus for the invariant tests of the parser and formatter
    corpus-nu_scripts = {
      url = "github:nushell/nu_scripts";
      flake = false;
    };
    corpus-nupm = {
      url = "github:nushell/nupm";
      flake = false;
    };
    corpus-tree-sitter-nu = {
      url = "github:nushell/tree-sitter-nu";
      flake = false;
    };
    corpus-kurokirasama-nushell_scripts = {
      url = "github:kurokirasama/nushell_scripts";
      flake = false;
    };

    # Formatter the benchmarks compare against
    nufmt = {
      url = "github:nushell/nufmt";
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

      nushell = forAllSystems (system: inputs.nixpkgs-nushell.legacyPackages.${system}.nushell);

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
            "benchmark"
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
        in
        {
          default = pkgs.mkShell {
            packages = [
              nushell.${system}
              rustToolchain.${system}
              pkgs.cargo-insta
              pkgs.cargo-nextest
              pkgs.hyperfine
              pkgs.time
              treefmtEval.${system}.config.build.wrapper
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
              pkgs.valgrind # Valgrind is broken on darwin.
            ];

            env = {
              NU_LIB_DIRS = "${inputs.nutest}";
              NUTEN_CORPUS = pkgs.runCommand "nuten-corpus" { } ''
                mkdir $out
                cp -r ${nushell.${system}.src}/crates/nu-std $out/nu-std
                cp -r ${inputs.corpus-nu_scripts} $out/nu_scripts
                cp -r ${inputs.corpus-nupm} $out/nupm
                cp -r ${inputs.corpus-tree-sitter-nu}/test/corpus $out/tree-sitter-nu
                cp -r ${inputs.corpus-kurokirasama-nushell_scripts} $out/kurokirasama-nushell_scripts
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

      packages = forAllSystems (system: {
        inherit (inputs.nufmt.packages.${system}) nufmt;
      });

      apps = forAllSystems (
        system:
        let
          pkgs = inputs.nixpkgs.legacyPackages.${system};
          nushellPkgs = inputs.nixpkgs-nushell.legacyPackages.${system};
        in
        {
          test-nu = {
            type = "app";
            program = pkgs.lib.getExe (
              nushellPkgs.writers.writeNuBin "nuten-test-nu" {
                makeWrapperArgs = [
                  "--set"
                  "NU_LIB_DIRS"
                  "${inputs.nutest}"
                  "--prefix"
                  "PATH"
                  ":"
                  (pkgs.lib.makeBinPath (
                    [
                      rustToolchain.${system}
                      pkgs.stdenv.cc
                      pkgs.coreutils
                    ]
                    ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
                      pkgs.valgrind
                      pkgs.util-linux
                    ]
                  ))
                ];
              } ../scripts/test-nu.nu
            );
            meta.description = "Run the tests written in nu";
          };
        }
      );

      checks = forAllSystems (system: {
        treefmt = treefmtEval.${system}.config.build.check (inputs.nixpkgs.lib.cleanSource ./..);
      });

      formatter = forAllSystems (system: treefmtEval.${system}.config.build.wrapper);
    };
}
