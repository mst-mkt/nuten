# nuten-line-index

The line index of [Nuten](https://github.com/mst-mkt/nuten), a formatter for Nushell.
It turns byte offsets in a source into line and column numbers, and has no dependencies.

## Usage

```rust
use nuten_line_index::located;

let places = located("let x = 1\nlet y = 2\n", [(14, "unused variable")]);
let (place, message) = places[0];
assert_eq!((place.line, place.column), (2, 5));
assert_eq!(message, "unused variable");
```

Lines and columns count from 1, and a column counts characters rather than bytes.

## Tests

```nushell
cargo nextest run --package nuten-line-index
```
