{
  inputs = {
    nuten = {
      url = "path:../..";
    };
    nixpkgs = {
      follows = "nuten/nixpkgs";
    };
    fenix = {
      follows = "nuten/fenix";
    };

    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
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

    # Corpus
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

    # Benchmark
    nufmt = {
      url = "github:nushell/nufmt";
    };
  };

  outputs =
    inputs:
    let
      inherit (inputs)
        nixpkgs
        nuten
        nutest
        nufmt
        ;

      systems = builtins.attrNames nuten.packages;
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});

      toolchainOf = pkgs: import ./toolchain.nix { inherit pkgs inputs; };
      treefmtOf =
        pkgs:
        import ./treefmt.nix {
          inherit pkgs inputs;
          toolchain = toolchainOf pkgs;
        };
    in
    {
      devShells = forAllSystems (
        pkgs:
        let
          toolchain = toolchainOf pkgs;
          treefmt = treefmtOf pkgs;
          gitHooks = import ./git-hooks.nix {
            inherit
              pkgs
              inputs
              toolchain
              treefmt
              ;
          };
          corpus = import ./corpus.nix {
            inherit pkgs inputs;
            inherit (toolchain) nushell;
          };
        in
        import ./shells.nix {
          inherit
            pkgs
            toolchain
            treefmt
            gitHooks
            corpus
            nutest
            ;
          nuten = nuten.packages.${pkgs.stdenv.hostPlatform.system}.default;
        }
      );

      packages = forAllSystems (pkgs: {
        inherit (toolchainOf pkgs) nushell;
        inherit (nufmt.packages.${pkgs.stdenv.hostPlatform.system}) nufmt;
      });

      formatter = forAllSystems (pkgs: (treefmtOf pkgs).config.build.wrapper);
    };
}
