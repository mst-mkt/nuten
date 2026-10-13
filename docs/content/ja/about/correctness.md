---
title: 正しさ
---

# 正しさ

## 解析と整形

Nuten は、コードを lossless CST に解析してから整形します。lossless CST は空白やコメントも含めて元の内容をすべて保持するため、解析の段階でトークンやコメントが失われる恐れがありません。

解析で構文エラーが一つでも見つかったファイルは、コードの構造を正しく読み取れたか分からないので、書き換えずにそのまま残します。

整形では CST のトークンをそのまま使い、変えるのはトークンの間の空白や改行、要素の区切り、コメントの末尾の空白だけです。ただし Nushell では、トークンの間に空白があるかや、改行の位置によって、コードの読み方が変わることがあります。そこで Nuten のパーサは、Nushell の組み込みコマンドの一覧や、キーワード、演算子の優先順位を持ち、Nushell と同じようにコードを読みます。そのうえで、空白や改行によって読み方が変わる位置には、整形で手を加えません。

## テスト

Nuten がコードを正しく解析しているか、整形でコードを壊していないかは、テストで確かめています。そのために、整形の前後で成り立つべき次の性質を不変条件として定め、さまざまな入力で検査しています。

1. トークンを失わないこと

   整形の前後で、空白、改行、コメントを除いたトークンが同じ順に並んでいることを確かめます。Nuten は設定の [`commas`](/ja/reference/settings#commas) に応じて `,` を足したり除いたりしますが、Nushell では要素を空白でも区切れるので、`,` の有無で意味は変わりません。そのため、要素を区切る `,` などは比べる対象から外しています。

2. コメントの位置を保つこと

   Nushell は、コメントの位置によって、それが何の説明かを決めます。

   ```nushell
   # Module description

   # Command description
   export def command [
       param: string # Parameter description
   ] {}
   ```

   1 行目のコメントは、空行で区切られているので、モジュールの説明になります。この空行を消すと、1 行目のコメントは `command` の説明に変わってしまいます。`param` の行末のコメントも、別の行に移すと `param` の説明ではなくなります。

   そこで、コメントごとに、次の項目を整形の前後で比べています。

   - 本文
   - コードの中での位置
   - 行末にあるか、単独の行にあるか
   - 文と文の間にあるコメントでは、直後に空行があるか

3. 整形を繰り返しても結果が変わらないこと

   整形の結果をもう一度整形し、結果が変わらないことを確かめます。その前に、整形の結果が構文エラーなく解析できることも確かめます。

これらの不変条件は、一つの設定で整形した結果だけでなく、設定を変えて整形した結果でも検査します。たとえば、行幅を 1 にしたり、タブでインデントしたり、`,` を付けなかったりします。こうすると、改行やインデント、`,` を扱う処理を幅広く試せます。

また、パーサにも不変条件を定めています。CST が入力の内容をすべて保持していることや、構文エラーを見落とさないことなどです。整形の不変条件を検査するときには、こちらもあわせて検査します。

不変条件を検査する処理は、整形については [`crates/nuten_format/src/invariants.rs`](https://github.com/mst-mkt/nuten/blob/main/crates/nuten_format/src/invariants.rs) に、パーサについては [`crates/nuten_parser/src/invariants.rs`](https://github.com/mst-mkt/nuten/blob/main/crates/nuten_parser/src/invariants.rs) にあります。

### snapshot

構文ごとに用意した手書きの入力で、解析の結果が意図せず変わっていないかを確かめるテストです。入力ごとに解析した CST を snapshot として記録しておき、テストのたびに記録と比べます。

あわせて、正しい構文の入力ではエラーを報告せず、誤った構文の入力ではエラーを報告することを確かめます。それぞれの入力では、パーサの不変条件も検査します。

テストは [`crates/nuten_parser/tests/parse.rs`](https://github.com/mst-mkt/nuten/blob/main/crates/nuten_parser/tests/parse.rs) に、テストデータは [`crates/nuten_parser/test_data`](https://github.com/mst-mkt/nuten/tree/main/crates/nuten_parser/test_data) にあります。

### fixtures

整形の規則ごとに、手書きの入力と期待する結果の組を fixtures として用意し、入力を整形したものが期待する結果と一致するかを確かめるテストです。設定によって結果が変わる規則には、設定ごとの組も用意しています。

あわせて、入力と期待する結果のどちらでも、整形でトークンが失われず、コメントの位置が保たれ、もう一度整形しても結果が変わらないことを確かめます。期待する結果は、整形済みの形になっていることも確かめます。

テストは [`crates/nuten_format/tests/fixtures.rs`](https://github.com/mst-mkt/nuten/blob/main/crates/nuten_format/tests/fixtures.rs) に、テストデータは [`crates/nuten_format/test_data`](https://github.com/mst-mkt/nuten/tree/main/crates/nuten_format/test_data) にあります。

### corpus

テスト用の corpus として、次のリポジトリから `.nu` と `.nuon` のファイルを集めています。手書きの入力だけでは補いきれない、実際のコードの書き方や規模を試せます。

| リポジトリ                                                                                            | 特徴                                                                                   | 量                              |
| ----------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- | ------------------------------- |
| [nushell/nushell/crates/nu-std](https://github.com/nushell/nushell/tree/main/crates/nu-std)           | Nushell 本体と一緒に保守され、多くのユーザーが使う、最新の構文のコード                 | 約 60 ファイル<br>約 8,000 行   |
| [nushell/nu_scripts](https://github.com/nushell/nu_scripts)                                           | 多くの人による、さまざまな書き方のコードや、古い構文のままのコード                     | 約 1,500 ファイル<br>約 17 万行 |
| [nushell/nupm](https://github.com/nushell/nupm)                                                       | 一つのプロジェクトとしてまとまったコードと、`.nuon` のファイル                         | 約 60 ファイル<br>約 2,000 行   |
| [nushell/tree-sitter-nu/test/corpus](https://github.com/nushell/tree-sitter-nu/tree/main/test/corpus) | 構文解析のテストのためのファイル<br>細かな構文を網羅したコードと、期待する構文木が並ぶ | 約 40 ファイル<br>約 1 万行     |
| [kurokirasama/nushell_scripts](https://github.com/kurokirasama/nushell_scripts)                       | 実際のユーザーによる大規模なスクリプト集                                               | 約 80 ファイル<br>約 3 万行     |

この corpus は、[fuzzing](#fuzzing) と [oracle](#oracle) のテストで入力に使います。集めるリポジトリの指定は [`nix/dev/corpus.nix`](https://github.com/mst-mkt/nuten/blob/main/nix/dev/corpus.nix) にあります。

また、corpus のすべてのファイルを整形して確かめるテストもあります。まず、Nushell も構文エラーとする数ファイルを除いて、どのファイルも Nuten が構文エラーなく解析できることを確かめます。そのうえで、整形でトークンが失われず、コメントの位置が保たれ、もう一度整形しても結果が変わらないことを確かめます。

テストは [`crates/nuten_format/tests/corpus.rs`](https://github.com/mst-mkt/nuten/blob/main/crates/nuten_format/tests/corpus.rs) にあります。

### fuzzing

機械的に作った大量の入力で Nuten を試すテストです。手書きの入力や corpus には現れにくい書き方や、構文エラーを含む壊れたコードも試せます。

入力は coverage-guided fuzzing の手法で作ります。corpus のスクリプトを出発点に、少しずつ変化させた入力を試し、Nuten のコードのうちまだ実行されていない部分を通ったものを残して、次の出発点にします。残した入力は次回の実行にも引き継ぐので、回を重ねるほど多くの書き方を試せます。

それぞれの入力では、次のことを確かめます。

- Nuten が異常終了せず、一つの入力の処理に時間がかかりすぎないこと
- 解析の結果が入力の内容をすべて保持し、構文エラーを見落とさないこと
- 構文エラーのない入力では、整形でトークンが失われず、コメントの位置が保たれ、もう一度整形しても結果が変わらないこと

テストは [`tasks/fuzz`](https://github.com/mst-mkt/nuten/tree/main/tasks/fuzz) にあります。

### oracle

Nushell を oracle にして、Nuten の解析や整形の結果を、Nushell による結果と比べるテストです。不変条件はトークンの並びやコメントの位置を見るだけなので、整形で Nushell の読み方が変わっていないかは、このテストで確かめます。また、Nuten のパーサは Nushell 本体とは別のものなので、Nushell と読み方が違う箇所もこのテストで見つけます。

入力は、corpus のスクリプト、Nushell のフォーマッタの issue で整形によって壊れたと報告されたコード、手書きの `.nuon` のファイルです。それぞれの入力では、次のことを確かめます。

- 整形の前後のコードを Nushell の `ast` コマンドで解析させ、構文木と、各トークンを何として読んだか (コマンド名、文字列、演算子など) が、位置の情報を除いて一致すること
- `.nuon` のファイルでは、整形の前後で Nushell が読み取る値が一致すること
- Nuten が構文エラーとしたファイルは、Nushell も構文エラーとすること
- 整形でトークンが失われず、コメントの位置が保たれ、もう一度整形しても結果が変わらないこと

これとは別に、パーサが持つ組み込みコマンドの一覧やシグネチャ、演算子の優先順位が、Nushell に問い合わせた結果と一致するかを、パーサの単体テストで確かめています。

比較を行うツールとその入力は [`tasks/oracle`](https://github.com/mst-mkt/nuten/tree/main/tasks/oracle) に、組み込みコマンドなどを Nushell に問い合わせる単体テストは [`crates/nuten_parser/src/grammar`](https://github.com/mst-mkt/nuten/tree/main/crates/nuten_parser/src/grammar) 以下にあります。

## 保証の範囲

Nuten が整形するのは、構文エラーなく解析できたファイルだけです。構文エラーが一つでもあるファイルには、手を加えません。

テストでは、整形でトークンが失われず、コメントの位置が保たれ、Nushell の読み方が変わらないことを確かめています。テストには、20 万行を超える実際のスクリプトと、それをもとに機械的に作った大量の入力を使っています。

ただし、これらの検査はテストの中でだけ行い、実際に整形するときには行いません。テストで試していない書き方や、テストで使っているものより新しい Nushell の構文では、整形がコードを壊すことや、Nuten が Nushell と違う読み方をすることがありえます。

整形でコードが壊れる例を見つけた場合は、[issue](https://github.com/mst-mkt/nuten/issues) で報告してください。
