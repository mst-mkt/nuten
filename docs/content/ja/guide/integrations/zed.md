---
title: Zed
---

# Zed

## インストール

> [!NOTE]
> 拡張機能は、まだ Zed の拡張機能レジストリに公開していません。

拡張機能は `nuten` を同梱しておらず、PATH から探して使います。先に `nuten` の [インストール](/ja/guide/installation) が必要です。

Zed の Nushell 対応は [Nu 拡張機能](https://github.com/zed-extensions/nu) が担っているので、こちらもインストールしてください。

Zed の拡張機能レジストリで公開しているので、拡張機能の画面で「Nuten」を検索してインストールできます。

### Nix

home-manager では、拡張機能の ID で追加できます。

```nix
{
  programs.zed-editor.extensions = [ "nu" "nuten-lsp" ];
}
```

## 設定

`settings.json` で Nuten を Nu のフォーマッタにします。

```json [settings.json]
{
  "languages": {
    "Nu": {
      "formatter": { "language_server": { "name": "nuten" } }
    }
  }
}
```

PATH にない `nuten` を使う場合は、次のようにパスを指定してください。

```json [settings.json]
{
  "lsp": {
    "nuten": {
      "binary": { "path": "/path/to/nuten", "arguments": ["lsp"] }
    }
  }
}
```

整形の設定は、Zed ではなく Nuten の [設定ファイル](/ja/guide/configuration) に書きます。
