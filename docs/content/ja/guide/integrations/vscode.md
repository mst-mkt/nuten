---
title: VS Code
---

# VS Code

## インストール

> [!NOTE]
> 拡張機能は、まだ Marketplace と Open VSX に公開していません。

拡張機能は `nuten` を同梱しておらず、PATH から探して使います。先に `nuten` の [インストール](/ja/guide/installation) が必要です。

[Marketplace](https://marketplace.visualstudio.com/items?itemName=mst-mkt.nuten-vscode) と [Open VSX](https://open-vsx.org/extension/mst-mkt/nuten-vscode) で公開しているので、拡張機能ビューで「Nuten」を検索するか、次のコマンドでインストールできます。

::: code-group

```nushell [VS Code]
code --install-extension mst-mkt.nuten-vscode
```

```nushell [VSCodium]
codium --install-extension mst-mkt.nuten-vscode
```

```nushell [Cursor]
cursor --install-extension mst-mkt.nuten-vscode
```

:::

### Nix

[mst-mkt/nuten](https://github.com/mst-mkt/nuten) か [nix-community/nix-vscode-extensions](https://github.com/nix-community/nix-vscode-extensions) の flake から拡張機能を追加できます。

::: code-group

```nix [mst-mkt/nuten]
{
  nixpkgs.overlays = [ nuten.overlays.default ];
  programs.vscode.profiles.default.extensions = [
    pkgs.vscode-extensions.mst-mkt.nuten-vscode
  ];
}
```

```nix [nix-community/nix-vscode-extensions]
{
  nixpkgs.overlays = [ nix-vscode-extensions.overlays.default ];
  programs.vscode.profiles.default.extensions = [
    pkgs.vscode-marketplace.mst-mkt.nuten-vscode
  ];
}
```

:::

## 設定

保存時に整形するには、`settings.json` で Nuten を既定のフォーマッタにします。

```json [settings.json]
{
  "[nushell]": {
    "editor.defaultFormatter": "mst-mkt.nuten-vscode",
    "editor.formatOnSave": true
  },
  "[nuon]": {
    "editor.defaultFormatter": "mst-mkt.nuten-vscode",
    "editor.formatOnSave": true
  }
}
```

拡張機能の設定は次の 2 つです。

| 項目                 | 説明                                                                                        | デフォルト    |
| -------------------- | ------------------------------------------------------------------------------------------- | ------------- |
| `nuten.path`         | `nuten` の実行ファイルのパス                                                                | PATH から探す |
| `nuten.trace.server` | 言語サーバーとの通信を出力パネルの `Nuten` に記録する<br>`"off"`, `"messages"`, `"verbose"` | `"off"`       |

整形の設定は、VS Code ではなく Nuten の [設定ファイル](/ja/guide/configuration) に書きます。
