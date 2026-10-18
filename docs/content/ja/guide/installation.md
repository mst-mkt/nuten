---
title: インストール
---

# インストール

> [!NOTE]
> インストール方法は今後追加する予定です。

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

[integrations/treefmt](/ja/guide/integrations/treefmt) を参照

### バイナリキャッシュ

ビルド済みのバイナリを `nuten.cachix.org` から取得できます。

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
