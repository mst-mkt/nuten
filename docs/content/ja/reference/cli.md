---
title: CLI
---

# CLI

```nushell
nuten <SUBCOMMAND>
```

| サブコマンド  | 説明                                  |
| ------------- | ------------------------------------- |
| `format`      | ファイルを整形する                    |
| `schema`      | 設定ファイルの JSON Schema を出力する |
| `lsp`         | 言語サーバーを起動する                |
| `completions` | シェルの補完スクリプトを出力する      |

## `nuten format`

```nushell
nuten format [FLAGS] [PATHS]...
```

| 引数    | 説明                                                                                                                                                                  | デフォルト |
| ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- |
| `PATHS` | 整形するファイルやディレクトリを指定する<br>ディレクトリを指定すると、配下の `.nu` と `.nuon` を整形する<br>`-` を指定すると stdin を整形し、結果を stdout に出力する | `.`        |

| フラグ                           | 説明                                                                                               |
| -------------------------------- | -------------------------------------------------------------------------------------------------- |
| `--config <PATH>`<br>`-c <PATH>` | 使う設定ファイルを指定する<br>指定しなければ、整形するファイルのディレクトリから親へ順に探す       |
| `--check`                        | ファイルを書き換えず、差分を stdout に出力する                                                     |
| `--list-different`<br>`-l`       | ファイルを書き換えず、差分のあるファイルのパスを 1 行ずつ stdout に出力する                        |
| `--reporter <github\|gitlab>`    | ファイルを書き換えず、差分とエラーを指定した CI の形式で stdout に出力する                         |
| `--stdin-filepath <PATH>`        | stdin の内容がどのファイルのものかを指定する<br>設定ファイルの探索と、出力に載せるファイル名に使う |

エラーは stderr に `error: <message>` の形で出力します。

最後に、処理したファイルの内訳を stderr に出力します。stdin を整形したときと `--list-different` では出力しません。

```
3 files: 2 need formatting (5 lines, 3 whitespace only), 1 unchanged
```

### exit code

| code | 整形       | 確認 (`--check`, `--list-different`, `--reporter`) |
| ---- | ---------- | -------------------------------------------------- |
| 0    | 成功       | 差分なし                                           |
| 1    |            | 差分あり                                           |
| 2    | エラーあり | エラーあり                                         |

エラーは、構文エラー, ファイルの読み書きの失敗, 設定や引数の誤り, 整形中の内部エラーです。

### 環境変数

| 変数             | 説明                                                            |
| ---------------- | --------------------------------------------------------------- |
| `NO_COLOR`       | 空でなければ出力に色を付けない<br>`CLICOLOR_FORCE` より優先する |
| `CLICOLOR_FORCE` | `0` でなければターミナル以外への出力にも色を付ける              |

### 例

```nushell
nuten format                                            # 整形する
nuten format --check                                    # 書き換えずに差分を出力する
nuten format --reporter github                          # 差分とエラーを GitHub Actions の注釈として出力する
$text | nuten format --stdin-filepath src/a.nu -        # stdin を src/a.nu の内容として整形する
```

## `nuten schema`

```nushell
nuten schema
```

設定ファイルの JSON Schema を stdout に出力します。

```nushell
nuten schema | save nuten.schema.json
```

## `nuten lsp`

```nushell
nuten lsp
```

stdin と stdout で通信する言語サーバーを起動します。

- `textDocument/formatting` のリクエストだけに対応する
- ドキュメントは全文で同期する
- 設定ファイルはドキュメントのパスから探し、リクエストごとに読み直す
- 構文エラーのあるドキュメントは整形せず、編集なしで返す
- shutdown を受けずに終了すると exit code は 1 になる

## `nuten completions`

```nushell
nuten completions <SHELL>
```

シェルの補完スクリプトを stdout に出力します。

| 引数    | 説明                   | 値                                                  |
| ------- | ---------------------- | --------------------------------------------------- |
| `SHELL` | 対象のシェルを指定する | `bash`, `elvish`, `fish`, `nu`, `powershell`, `zsh` |

```nushell
nuten completions nu | save nuten-completions.nu
```
