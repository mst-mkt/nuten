# nuten-doc

The document IR and printer of [Nuten](https://github.com/mst-mkt/nuten), a formatter for Nushell.
It knows nothing about Nushell, and its only dependency is [unicode-width](https://crates.io/crates/unicode-width), which measures the width of text.

## Usage

A formatter builds a `Document` with `Builder::build`, and `print` lays it out within the line width of `PrinterOptions`.

```rust
use nuten_doc::{
    Build, Builder, IndentStyle, LineEnding, PrinterOptions, print, soft_line_break,
    soft_line_break_or_space, text,
};

let document = Builder::build(|b| {
    b.group(|b| {
        b.push(text("["));
        b.indent(|b| {
            b.extend([
                soft_line_break(),
                text("1,"),
                soft_line_break_or_space(),
                text("2"),
            ]);
        });
        b.extend([soft_line_break(), text("]")]);
    });
});
let options = PrinterOptions {
    line_width: 6,
    indent_style: IndentStyle::Space,
    indent_width: 4,
    line_ending: LineEnding::Lf,
};
assert_eq!(print(&document, &options), "[1, 2]");

let narrow = PrinterOptions {
    line_width: 5,
    ..options
};
assert_eq!(print(&document, &narrow), "[\n    1,\n    2\n]");
```

Text and line breaks are functions that return an element.
Containers such as groups and indents are methods of the `Build` trait, and take a closure that writes their contents.

| Element                                                                                  | Meaning                                                                          |
| ---------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| `text`                                                                                   | Text whose line breaks take the line ending of `PrinterOptions`                  |
| `exact_text`                                                                             | Text whose line breaks stay as written, such as the contents of a string         |
| `space`                                                                                  | A space                                                                          |
| `soft_line_break`                                                                        | A line break if the group breaks, and nothing otherwise                          |
| `soft_line_break_or_space`                                                               | A line break if the group breaks, and a space otherwise                          |
| `hard_line_break`                                                                        | A line break that breaks the groups around it                                    |
| `empty_line`, `empty_lines`                                                              | A line break followed by blank lines                                             |
| `expand_parent`                                                                          | A mark that breaks the groups around it                                          |
| `b.group`                                                                                | Elements printed on one line if they fit, and broken otherwise                   |
| `b.group_with_id`                                                                        | A group with an id, so that other elements can depend on whether it breaks       |
| `b.indent`, `b.indent_levels`, `b.align`                                                 | Elements indented by levels, or aligned by columns                               |
| `b.indent_if_group_breaks`, `b.indent_levels_if_group_breaks`, `b.align_if_group_breaks` | Elements indented or aligned only when the named group breaks                    |
| `b.flat_or`                                                                              | Elements written flat, or other elements in their place when they cannot be flat |
| `b.if_break`                                                                             | Elements that differ between a broken group and a flat one                       |
| `b.line_suffix`                                                                          | Elements deferred to the end of the line, such as trailing comments              |
| `line_suffix_boundary`                                                                   | A point where the deferred elements are printed                                  |

## Tests

```nushell
cargo nextest run --package nuten-doc
```
