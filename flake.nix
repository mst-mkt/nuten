{
  inputs = {
    nixpkgs = {
      url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { nixpkgs, fenix, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forAllSystems =
        f:
        nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system} fenix.packages.${system});
    in
    {
      devShells = forAllSystems (
        pkgs: fenixPkgs: {
          default = pkgs.mkShell {
            packages = [
              (
                with fenixPkgs;
                combine [
                  stable.cargo
                  stable.rustc
                  stable.clippy
                  stable.rust-src
                  stable.rust-analyzer
                  latest.rustfmt
                ]
              )
            ];
          };
        }
      );
    };
}
