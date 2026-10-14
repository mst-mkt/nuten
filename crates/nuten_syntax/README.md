# nuten-syntax

The syntax tree types of [Nuten](https://github.com/mst-mkt/nuten), a formatter for Nushell.
It defines the kinds of tokens and nodes, and the tree built on [rowan](https://crates.io/crates/rowan), its only dependency.

## Usage

`SyntaxKind` lists every kind of token and node.
`NuLanguage` ties `SyntaxKind` to rowan, and `SyntaxNode`, `SyntaxToken`, and `SyntaxElement` are the rowan types for Nushell.
`ast` gives typed views over the nodes, so that code reads the parts of a node by name.

## Tests

```nushell
cargo nextest run --package nuten-syntax
```
