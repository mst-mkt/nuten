---
title: Helix
---

# Helix

`languages.toml` に `nuten lsp` を言語サーバーとして登録し、保存時に整形します。Helix は PATH 上の `nuten` を呼び出すので、先に `nuten` の [インストール](/ja/guide/installation) が必要です。

```toml [languages.toml]
[language-server.nuten]
command = "nuten"
args = ["lsp"]

[[language]]
name = "nu"
language-servers = ["nu-lsp", "nuten"]
auto-format = true
```

`language-servers` は Helix の既定の一覧を置き換えるので、Nushell の言語サーバー `nu-lsp` も並べて書きます。`nu-lsp` は整形に対応していないので、整形は Nuten が担います。

PATH にない `nuten` を使う場合は、`command` にそのパスを書いてください。

## Nix

home-manager では、`programs.helix.languages` に `languages.toml` の内容を書きます。

```nix
{
  programs.helix = {
    enable = true;
    languages = {
      language-server.nuten = {
        command = "nuten";
        args = [ "lsp" ];
      };
      language = [
        {
          name = "nu";
          language-servers = [ "nu-lsp" "nuten" ];
          auto-format = true;
        }
      ];
    };
  };
}
```
