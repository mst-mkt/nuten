# benchmark

The benchmarks of [Nuten](https://github.com/mst-mkt/nuten), a formatter for Nushell.
They are Nushell scripts that run the `nuten` binary on files from the corpus.

## Usage

`main.nu` runs each task below as a subcommand, and `nu tasks/benchmark/main.nu --help` lists them.
Each subcommand takes the corpus first.
The `nix/dev` devShell sets `NUTEN_CORPUS` to the corpus.
Without Nix, `nu scripts/prepare-corpus.nu path/to/corpus` assembles it.

### base

Compares the instructions and heap usage of two `nuten` builds under valgrind, on each input.
It needs valgrind, timeout, and taskset, so it runs on Linux only.

```nushell
nu tasks/benchmark/main.nu base path/to/corpus path/to/base/nuten path/to/head/nuten
```

CI runs it on pull requests, and posts the result as a comment.

### nufmt

Compares the time of `nuten` and nufmt with hyperfine, on the files both format without breaking, which `nuten-oracle` finds.
It needs timeout and hyperfine.

```nushell
nu tasks/benchmark/main.nu nufmt path/to/corpus path/to/nuten path/to/nufmt path/to/nuten-oracle
```

### select

Picks the inputs of `base` from the corpus, such as the largest file and the one with the most pipes.
Run it again after updating the corpus.

```nushell
nu tasks/benchmark/main.nu select path/to/corpus path/to/nuten | save -f tasks/benchmark/inputs.nuon
```

## Tests

`vp run test:nu` runs the [nutest](https://github.com/vyadh/nutest) suites in `tests`, on Linux only.
