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
    nuten = {
      # TODO: change to github: when publishing the repository
      url = "git+ssh://git@github.com/mst-mkt/nuten";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
    };
    nushell = {
      # Not following nixpkgs, since this release is built and tested against its own.
      url = "github:nushell/nushell/0.116.0?dir=scripts/nix";
    };
    nutest = {
      url = "github:vyadh/nutest/v1.2.0";
      flake = false;
    };
    nix-vite-plus = {
      url = "github:ryoppippi/nix-vite-plus";
      inputs.nixpkgs.follows = "nixpkgs";
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
          targets.wasm32-wasip2.stable.rust-std
          latest.rustfmt
        ]
      );

      nushell = forAllSystems (
        system:
        # Its own tests take long and tell nothing of how nu parses.
        inputs.nushell.packages.${system}.default.overrideAttrs { doCheck = false; }
      );
      nodejs = forAllSystems (system: inputs.nixpkgs.legacyPackages.${system}.nodejs-slim_26);
      pnpm = forAllSystems (
        system: inputs.nixpkgs.legacyPackages.${system}.pnpm_11.override { nodejs-slim = nodejs.${system}; }
      );
      vite-plus = forAllSystems (
        system:
        let
          vp = inputs.nix-vite-plus.packages.${system}.vp.override { nodejs = nodejs.${system}; };
          wanted = (inputs.nixpkgs.lib.importJSON ../../package.json).devDependencies.vite-plus;
        in
        assert inputs.nixpkgs.lib.assertMsg (
          vp.version == wanted
        ) "vite-plus in nix/dev/flake.lock (${vp.version}) differs from package.json (${wanted})";
        vp
      );

      treefmtEval = forAllSystems (
        system:
        inputs.treefmt-nix.lib.evalModule inputs.nixpkgs.legacyPackages.${system} {
          imports = [ inputs.nuten.treefmtModule ];

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
              excludes = [ "pnpm-lock.yaml" ];
            };
            nuten = {
              enable = true;
              excludes = [
                "crates/*/test_data/**"
                "tasks/oracle/test_data/**"
              ];
            };
          };

          settings.formatter = {
            oxfmt.command = "${vite-plus.${system}}/node_modules/oxfmt/bin/oxfmt";
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
            "lsp"
            "vscode"
            "zed"
            "assets"
          ];
        in
        inputs.git-hooks.lib.${system}.run {
          src = ./../..;
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
              nodejs.${system}
              pnpm.${system}
              vite-plus.${system}
              pkgs.cargo-insta
              pkgs.cargo-nextest
              pkgs.hyperfine
              pkgs.time
              treefmtEval.${system}.config.build.wrapper
              inputs.nuten.packages.${system}.default
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

          vscode = pkgs.mkShell {
            packages = [
              rustToolchain.${system}
              nodejs.${system}
              pnpm.${system}
              vite-plus.${system}
            ];
            env = pkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
              NUTEN_VSCODE = pkgs.writeShellScript "codium-headless" ''
                unset WAYLAND_DISPLAY
                exec ${pkgs.xvfb-run}/bin/xvfb-run --auto-servernum \
                  ${pkgs.vscodium}/lib/vscode/codium --ozone-platform=x11 "$@"
              '';
            };
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
        in
        {
          test-nu = {
            type = "app";
            program = pkgs.lib.getExe (
              # Not writeNuBin, which runs the nushell of nixpkgs.
              pkgs.writers.makeScriptWriter {
                interpreter = "${pkgs.lib.getExe nushell.${system}} --no-config-file";
                makeWrapperArgs = [
                  "--set"
                  "NU_LIB_DIRS"
                  "${inputs.nutest}"
                  "--prefix"
                  "PATH"
                  ":"
                  (pkgs.lib.makeBinPath (
                    [
                      nushell.${system}
                      rustToolchain.${system}
                      pkgs.stdenv.cc
                      pkgs.coreutils
                      pkgs.hyperfine
                      pkgs.time
                      pkgs.git
                    ]
                    ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [
                      pkgs.valgrind
                      pkgs.util-linux
                    ]
                  ))
                ];
              } "/bin/nuten-test-nu" ../../scripts/test-nu.nu
            );
            meta.description = "Run the tests written in nu";
          };
        }
      );

      checks = forAllSystems (system: {
        treefmt = treefmtEval.${system}.config.build.check (inputs.nixpkgs.lib.cleanSource ./../..);
      });

      formatter = forAllSystems (system: treefmtEval.${system}.config.build.wrapper);
    };
}
