---
title: Correctness
---

# Correctness

## Parsing and formatting

Nuten parses code into a lossless CST before formatting it. A lossless CST keeps everything from the original, including whitespace and comments, so no token or comment can be lost while parsing.

If parsing finds even one syntax error in a file, the file is left untouched, since Nuten cannot be sure it has read the structure of the code correctly.

Formatting reuses the tokens in the CST as they are, and changes only the whitespace and line breaks between tokens, the separators between items, and trailing whitespace in comments. In Nushell, though, whether tokens touch and where lines break can change how code is read. So Nuten's parser knows Nushell's built-in commands, keywords, and operator precedence, and reads code the way Nushell does. Where changing whitespace or a line break would change how code is read, formatting leaves it as it is.

## Tests

Tests check that Nuten parses code correctly and that formatting does not break it. To do so, they check a wide range of inputs against the following invariants, which must hold before and after formatting.

1. No tokens are lost

   Apart from whitespace, line breaks, and comments, the same tokens must appear in the same order before and after formatting. Nuten adds or removes `,` depending on the [`commas`](/reference/settings#commas) setting, but Nushell also accepts spaces between items, so adding or removing a `,` does not change the meaning. For that reason, `,` and other separators between items are left out of the comparison.

2. Comments stay where they are

   Nushell decides what a comment describes from where it stands.

   ```nushell
   # Module description

   # Command description
   export def command [
       param: string # Parameter description
   ] {}
   ```

   The first comment is set apart by a blank line, so it describes the module. Remove that blank line, and the first comment describes `command` instead. Likewise, the comment at the end of the `param` line no longer describes `param` once it moves to a line of its own.

   So for each comment, the following are compared before and after formatting.

   - Its text
   - Its position in the code
   - Whether it ends a line or stands on a line of its own
   - For a comment between statements, whether a blank line follows it

3. Formatting again changes nothing

   Formatting the output once more must give the same output. Before that, the output must parse without syntax errors.

These invariants are checked not only on output formatted with one configuration, but also on output formatted with others, such as a line width of 1, tab indentation, or no commas. This exercises the handling of line breaks, indentation, and `,` more broadly.

The parser has invariants of its own too, such as the CST keeping everything in the input and no syntax error going unreported. They are checked along with the formatter's.

The invariant checks are in [`crates/nuten_format/src/invariants.rs`](https://github.com/mst-mkt/nuten/blob/main/crates/nuten_format/src/invariants.rs) for the formatter and [`crates/nuten_parser/src/invariants.rs`](https://github.com/mst-mkt/nuten/blob/main/crates/nuten_parser/src/invariants.rs) for the parser.

### Snapshots

This test uses hand-written inputs for each kind of syntax to check that parsing results do not change unintentionally. The CST parsed from each input is recorded as a snapshot, and compared against the record on every run.

It also checks that no errors are reported for inputs with valid syntax, and that errors are reported for inputs with invalid syntax. The parser invariants are checked on every input as well.

The test is in [`crates/nuten_parser/tests/parse.rs`](https://github.com/mst-mkt/nuten/blob/main/crates/nuten_parser/tests/parse.rs), and its data in [`crates/nuten_parser/test_data`](https://github.com/mst-mkt/nuten/tree/main/crates/nuten_parser/test_data).

### Fixtures

For each formatting rule, pairs of a hand-written input and its expected output are kept as fixtures, and this test checks that formatting the input gives the expected output. Rules whose output depends on the configuration have a pair for each configuration.

It also checks, on both the input and the expected output, that formatting loses no tokens, keeps comments where they are, and changes nothing when run again. The expected output must also already be formatted.

The test is in [`crates/nuten_format/tests/fixtures.rs`](https://github.com/mst-mkt/nuten/blob/main/crates/nuten_format/tests/fixtures.rs), and its data in [`crates/nuten_format/test_data`](https://github.com/mst-mkt/nuten/tree/main/crates/nuten_format/test_data).

### Corpus

For testing, a corpus of `.nu` and `.nuon` files is gathered from the repositories below. It covers the styles and sizes of real-world code that hand-written inputs alone cannot.

| Repository                                                                                            | Notes                                                                                                | Size                                     |
| ----------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- | ---------------------------------------- |
| [nushell/nushell/crates/nu-std](https://github.com/nushell/nushell/tree/main/crates/nu-std)           | Code in the latest syntax, maintained alongside Nushell and used by many users                       | About 60 files<br>about 8,000 lines      |
| [nushell/nu_scripts](https://github.com/nushell/nu_scripts)                                           | Code by many people in many styles, some still in older syntax                                       | About 1,500 files<br>about 170,000 lines |
| [nushell/nupm](https://github.com/nushell/nupm)                                                       | Code organized as one project, plus `.nuon` files                                                    | About 60 files<br>about 2,000 lines      |
| [nushell/tree-sitter-nu/test/corpus](https://github.com/nushell/tree-sitter-nu/tree/main/test/corpus) | Test files for a parser<br>Code covering fine details of the syntax, alongside expected syntax trees | About 40 files<br>about 10,000 lines     |
| [kurokirasama/nushell_scripts](https://github.com/kurokirasama/nushell_scripts)                       | A large collection of scripts by a real user                                                         | About 80 files<br>about 30,000 lines     |

The corpus serves as input to the [fuzzing](#fuzzing) and [oracle](#oracle) tests. The repositories are listed in [`nix/dev/corpus.nix`](https://github.com/mst-mkt/nuten/blob/main/nix/dev/corpus.nix).

There is also a test that formats every file in the corpus. First, it checks that Nuten parses every file without syntax errors, except for a few files that Nushell also rejects. Then it checks that formatting loses no tokens, keeps comments where they are, and changes nothing when run again.

The test is in [`crates/nuten_format/tests/corpus.rs`](https://github.com/mst-mkt/nuten/blob/main/crates/nuten_format/tests/corpus.rs).

### Fuzzing

This test tries Nuten on a large number of generated inputs. It covers styles that rarely show up in hand-written inputs or the corpus, as well as broken code with syntax errors.

Inputs are generated by coverage-guided fuzzing. Starting from the scripts in the corpus, the fuzzer tries slightly changed inputs, keeps those that reach parts of Nuten not run before, and uses them as new starting points. The kept inputs carry over to the next run, so the more it runs, the more kinds of code it tries.

For each input, it checks the following.

- Nuten does not crash, and does not take too long on any single input
- The parse result keeps everything in the input and misses no syntax error
- For input without syntax errors, formatting loses no tokens, keeps comments where they are, and changes nothing when run again

The test is in [`tasks/fuzz`](https://github.com/mst-mkt/nuten/tree/main/tasks/fuzz).

### Oracle

This test uses Nushell as an oracle, comparing what Nuten parses and formats against Nushell itself. The invariants only look at the order of tokens and the position of comments, so whether formatting changes how Nushell reads the code is checked here. Since Nuten's parser is separate from Nushell's, this test is also where differences in how the two read code turn up.

The inputs are the scripts in the corpus, code reported in Nushell formatter issues as broken by formatting, and hand-written `.nuon` files. For each input, it checks the following.

- Nushell's `ast` command gives the same syntax tree for the code before and after formatting, apart from positions, and reads each token as the same kind, such as a command name, a string, or an operator
- For a `.nuon` file, Nushell reads the same value before and after formatting
- Any file Nuten rejects with a syntax error is also rejected by Nushell
- Formatting loses no tokens, keeps comments where they are, and changes nothing when run again

Separately, the parser's unit tests check that its list of built-in commands, their signatures, and operator precedence match what Nushell reports.

The comparison tool and its inputs are in [`tasks/oracle`](https://github.com/mst-mkt/nuten/tree/main/tasks/oracle), and the unit tests that query Nushell for built-in commands and the like are under [`crates/nuten_parser/src/grammar`](https://github.com/mst-mkt/nuten/tree/main/crates/nuten_parser/src/grammar).

## What is guaranteed

Nuten formats only files that parse without syntax errors. A file with even one syntax error is left untouched.

Tests check that formatting keeps tokens and the positions of comments, and does not change how Nushell reads the code. Their inputs include more than 200,000 lines of real-world scripts and a large body of code generated from them.

These checks run only in tests, though, not when you format your code. With code written in ways the tests have not covered, or with syntax from a newer Nushell than the tests use, formatting may break code, or Nuten may read code differently from Nushell.

If you find code that formatting breaks, please report it in an [issue](https://github.com/mst-mkt/nuten/issues).
