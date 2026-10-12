---
title: CI
---

# CI

## GitHub Actions

> [!NOTE]
> setup-nuten は、まだ公開していません。

[setup-nuten](https://github.com/mst-mkt/setup-nuten) で `nuten` を入れ、`--reporter github` で結果を注釈として出力します。

```yaml [.github/workflows/format.yaml]
on:
  pull_request:
  push:
    branches: [main]

jobs:
  format:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
      - uses: mst-mkt/setup-nuten@{sha} # {version}
      - run: nuten format --reporter github
```

`--reporter github` はファイルを書き換えません。整形が必要な行とエラーが注釈になり、プルリクエストの差分に表示されます。整形が必要なファイルやエラーがあると、ジョブは失敗します。

## GitLab CI

> [!NOTE]
> インストール用のスクリプトは、まだ公開していません。

インストール用のスクリプトで `nuten` を入れ、`--reporter gitlab` で結果を Code Quality のレポートにします。

```yaml [.gitlab-ci.yml]
format:
  script:
    - curl -fsSL https://nuten.keito.dev/install.sh | sh
    - export PATH="$HOME/.local/bin:$PATH"
    - nuten format --reporter gitlab > gl-code-quality-report.json
  artifacts:
    when: always
    reports:
      codequality: gl-code-quality-report.json
```

`--reporter gitlab` はファイルを書き換えず、レポートを stdout に出力します。レポートには整形が必要な行とエラーが載り、マージリクエストに表示されます。整形が必要なファイルやエラーがあるとジョブは失敗するので、`when: always` で失敗したときもレポートを保存します。
