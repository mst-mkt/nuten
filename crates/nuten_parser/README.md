# nuten-parser

The lexer and parser of [Nuten](https://github.com/mst-mkt/nuten), a formatter for Nushell.
It parses Nushell source into a lossless syntax tree of [nuten-syntax](../nuten_syntax), which keeps every character including whitespace and comments.

## Usage

```rust
use nuten_parser::parse;

let src = "ls | where size > 1kb";
let parsed = parse(src);
assert!(parsed.errors().is_empty());
assert_eq!(parsed.syntax().to_string(), src);
```

`parse` always returns a tree, and reports syntax errors alongside it.
`invariants` reports where the tokens or the tree break the invariants, such as a tree that does not round-trip to the source.

## Structure

The grammar lives in `src/grammar`, mostly one file for each kind of syntax.
`src/grammar/builtin.rs` holds the built-in commands of nu, so that the parser reads them as nu does.

## Dependencies

| Crate                                         | Used for                      |
| --------------------------------------------- | ----------------------------- |
| [nuten-syntax](../nuten_syntax)               | The kinds of tokens and nodes |
| [rowan](https://crates.io/crates/rowan)       | Building the tree             |
| [insta](https://crates.io/crates/insta) (dev) | The snapshots of the trees    |

## Tests

`test_data/ok` holds inputs that parse without errors, and `test_data/err` holds inputs that have errors, each grouped by the kind of syntax.
Each input is checked against the invariants, and keeps its tree as an [insta](https://insta.rs) snapshot next to it, so that a change to the tree shows up for review.

Some unit tests check the built-in commands, their signatures, and operator precedence against nu.
Without nu on the PATH, these tests fail unless `NUTEN_TEST_SKIP_NU` is set.

```nushell
cargo nextest run --package nuten-parser
cargo insta review
```
