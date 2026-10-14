# benchmark

The benchmarks of [Nuten](https://github.com/mst-mkt/nuten), a formatter for Nushell.
They are Nushell scripts that run the `nuten` binary on files from the corpus.

## Usage

Each of the scripts below takes the corpus first.
The `nix/dev` devShell sets `NUTEN_CORPUS` to the corpus.
Without Nix, `nu scripts/prepare-corpus.nu path/to/corpus` assembles it.

### base.nu

Compares the instructions and heap usage of two `nuten` builds under valgrind, on each input.
It needs valgrind, timeout, and taskset, so it runs on Linux only.

```nushell
nu tasks/benchmark/base.nu path/to/corpus path/to/base/nuten path/to/head/nuten
```

CI runs it on pull requests, and posts the result as a comment.

### nufmt.nu

Compares the time of `nuten` and nufmt with hyperfine, on the files both format without breaking, which `nuten-oracle` finds.
It needs timeout and hyperfine.

```nushell
nu tasks/benchmark/nufmt.nu path/to/corpus --nuten path/to/nuten --nufmt path/to/nufmt --oracle path/to/nuten-oracle
```

### select.nu

Picks the inputs of `base.nu` from the corpus, such as the largest file and the one with the most pipes.
Run it again after updating the corpus.

```nushell
nu tasks/benchmark/select.nu path/to/corpus path/to/nuten | save -f tasks/benchmark/inputs.nuon
```

## Tests

`vp run test:nu` runs the [nutest](https://github.com/vyadh/nutest) suites in `tests`, on Linux only.
