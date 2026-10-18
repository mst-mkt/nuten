---
title: Installation
---

# Installation

> [!NOTE]
> More installation methods are coming.

## Cargo

```nushell
cargo install --git https://github.com/mst-mkt/nuten nuten
```

## Nix

### nix profile

```nushell
nix profile install github:mst-mkt/nuten
```

### flake

```nix
{
  inputs = {
    nuten.url = "github:mst-mkt/nuten";
  };

  outputs = { nixpkgs, nuten, ... }: {
    devShells.x86_64-linux.default = nixpkgs.legacyPackages.x86_64-linux.mkShell {
      packages = [ nuten.packages.x86_64-linux.default ];
    };
  };
}
```

### treefmt-nix

See [integrations/treefmt](/guide/integrations/treefmt)

### Binary cache

Pre-built binaries can be fetched from `nuten.cachix.org`.

```nushell
cachix use nuten
```

```nix
{
  nix.settings = {
    extra-substituters = [ "https://nuten.cachix.org" ];
    extra-trusted-public-keys = [ "nuten.cachix.org-1:UQhrCY0F6u/dOjkl0oySJqXRlG0gbYXl8o7AQ4bmcDs=" ];
  };
}
```
