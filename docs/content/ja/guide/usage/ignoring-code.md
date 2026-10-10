---
title: 無効化
---

# 無効化

コメントで一部の整形を無効化できます。手で揃えた表などをそのまま残したい場合に便利です。

ファイル単位で整形しない場合は、設定の [`exclude`](/ja/guide/usage/configuration) を使います。

## `# nuten-ignore`

行頭に置くと、直後の文や要素をそのまま残します。

::: compare

```nushell [Before]
# nuten-ignore
let sizes = [[name,     bytes];
             [small,       10]
             [medium,    1000]
             [large,   100000]]

let names = [[id, name];
             [1, a]
             [2, b]]
```

```nushell [After]
# nuten-ignore
let sizes = [[name,     bytes];
             [small,       10]
             [medium,    1000]
             [large,   100000]]

let names = [[id, name];
    [1, a]
    [2, b]
]
```

:::

行末に置くと、その行の文や要素をそのまま残します。

::: compare

```nushell [Before]
let cross = [1 0 1  0 1 0  1 0 1] # nuten-ignore
let plus = [0 1 0  1 1 1  0 1 0]
```

```nushell [After]
let cross = [1 0 1  0 1 0  1 0 1] # nuten-ignore
let plus = [0, 1, 0, 1, 1, 1, 0, 1, 0]
```

:::
