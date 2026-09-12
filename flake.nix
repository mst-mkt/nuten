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
          stable.cargo
          stable.rustc
          stable.clippy
          stable.rust-src
          stable.rust-analyzer
          latest.rustfmt
        ]
      );

      treefmtEval = forAllSystems (
        system:
        inputs.treefmt-nix.lib.evalModule inputs.nixpkgs.legacyPackages.${system} {
          projectRootFile = "flake.nix";

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
        inputs.git-hooks.lib.${system}.run {
          src = inputs.self;
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
              rustToolchain.${system}
              treefmtEval.${system}.config.build.wrapper
            ];
            inherit (preCommit.${system}) shellHook;
          };
        }
      );

      checks = forAllSystems (system: {
        treefmt = treefmtEval.${system}.config.build.check inputs.self;
      });

      formatter = forAllSystems (system: treefmtEval.${system}.config.build.wrapper);
    };
}
