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
    {
      nixpkgs,
      crane,
      fenix,
      ...
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});

      toolchainOf = pkgs: import ./nix/toolchain.nix { inherit pkgs crane fenix; };
      nutenOf = pkgs: pkgs.callPackage ./nix/nuten.nix { inherit (toolchainOf pkgs) craneLib; };
      vscodeExtensionOf =
        pkgs: pkgs.callPackage ./nix/vscode-extension.nix { inherit (toolchainOf pkgs) pnpm; };

      packages = forAllSystems (pkgs: {
        default = nutenOf pkgs;
        vscode-extension = vscodeExtensionOf pkgs;
      });
    in
    {
      inherit packages;

      overlays.default = final: prev: {
        nuten = nutenOf final;
        vscode-extensions = prev.vscode-extensions // {
          mst-mkt = (prev.vscode-extensions.mst-mkt or { }) // {
            nuten-vscode = vscodeExtensionOf final;
          };
        };
      };

      treefmtModule = import ./nix/treefmt-module.nix { inherit nutenOf; };

      checks = packages;
    };
}
