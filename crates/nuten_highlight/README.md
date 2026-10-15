# nuten-highlight

Syntax highlighting of Nushell, built on [nuten-parser](../nuten_parser).
Only the [playground](../../playground) uses it for now.

## Usage

```rust
use nuten_highlight::{TokenType, highlight};

let root = nuten_parser::parse("ls -la").syntax();
let types: Vec<_> = highlight(&root).into_iter().map(|(_, token_type)| token_type).collect();
assert_eq!(types, [TokenType::Function, TokenType::Flag]);
```

The ranges are byte offsets into the source, come in order, and never overlap.
The types are the semantic token types of LSP, and `Flag`.

## Dependencies

| Crate                           | Used for                                          |
| ------------------------------- | ------------------------------------------------- |
| [nuten-parser](../nuten_parser) | Telling keywords and numbers as nu does           |
| [nuten-syntax](../nuten_syntax) | Reading the kinds of tokens and nodes in the tree |

## Tests

```nushell
cargo nextest run --package nuten-highlight
```

`tests/ranges.rs` highlights every fixture of the parser and the formatter, and checks that the ranges come in order, each within one token.
