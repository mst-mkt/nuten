---
title: Ignoring code
---

# Ignoring code

A comment turns formatting off for a piece of code. Use it to keep things like a hand-aligned table as they are.

To leave whole files alone, use [`exclude`](/guide/configuration) in the configuration.

## `# nuten-ignore`

On its own line, the comment keeps the statement or element that follows as it is.

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

At the end of a line, it keeps the statement or element on that line as it is.

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
