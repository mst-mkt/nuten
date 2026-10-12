---
title: treefmt
---

# treefmt

## treefmt-nix

`treefmtModule` を読み込むと、treefmt-nix で `programs.nuten` を使えるようになります。

```nix
treefmt-nix.lib.evalModule pkgs {
  imports = [ inputs.nuten.treefmtModule ]; # github:mst-mkt/nuten
  programs.nuten = {
    enable = true;
    excludes = [ "tests/fixtures/*" ];
    settings = {
      line_width = 80;
    };
  };
}
```

`settings` には [設定ファイル](/ja/guide/configuration) と同じ内容を書きます。`include` と `exclude` は `settings` には書かず、treefmt 側の `includes` と `excludes` に書きます。

`settings` を書くと、プロジェクトの設定ファイルは使われません。treefmt を経由せずに `nuten format` やエディタからも Nuten を使うときは、`settings` は書かずに、プロジェクトの [設定ファイル](/ja/guide/configuration) に設定をまとめてください。このとき、整形するファイルは `programs.nuten` の `includes` と `excludes` に加えて、設定ファイルの `include` と `exclude` にも書いてください。

ビルド済みのバイナリは [バイナリキャッシュ](/ja/guide/installation#バイナリキャッシュ) から取得できます。

## treefmt.toml

treefmt は PATH 上の `nuten` を使うので、先に `nuten` の [インストール](/ja/guide/installation) が必要です。

```toml [treefmt.toml]
[formatter.nuten]
command = "nuten"
options = ["format"]
includes = ["*.nu", "*.nuon"]
```

整形の設定は、プロジェクトの [設定ファイル](/ja/guide/configuration) に書きます。

整形するファイルは `[formatter.nuten]` の `includes` と `excludes` に書きます。treefmt を経由せずに `nuten format` やエディタからも Nuten を使うときは、設定ファイルの `include` と `exclude` にも同じ指定を書いてください。
