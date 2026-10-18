---
title: Performance
---

# Performance

Nuten is compared with [nufmt](https://github.com/nushell/nufmt) on a [corpus](/about/correctness#corpus) of real-world scripts.

```benchmark
overview
```

## Formatting a directory

`nuten format --list-different` and `nufmt --dry-run` both format every file in a directory and report the files that would change. Each time is the median of the runs. Both formatters process files in parallel, so times are shown both with all CPUs and with a single CPU.

To keep the comparison fair, only files that both formatters can format without failing or changing the AST are timed. The rest are listed under [Files left out](#files-left-out).

### Overall

```benchmark
directory all
```

### nu-std

```benchmark
directory nu-std
```

## Formatting a single file

The values are the median number of CPU instructions needed to format a single file read from stdin. Editors and the language server format files this way.

Formatting a single file takes so little time that starting the process takes up most of the wall-clock time. Instructions are counted with valgrind instead, since the count does not depend on how fast or busy the machine is.

```benchmark
single
```

## Memory

The values are the peak memory use of each formatter. Single file is the median over the files formatted one at a time, and Overall is formatting every file in the directory on all CPUs.

```benchmark
memory
```

## Binary size

Each executable is measured as installed.

```benchmark
size
```

## Files left out

A file is compared only when both formatters can format it and Nushell parses the output into the same AST as the input. Any other file is left out for both formatters. The Correctness figure at the top counts the files left out because the output changes the AST.

```benchmark
excluded
```

## Conditions

```benchmark
conditions
```

## Reproducing

The benchmark is in [`tasks/benchmark`](https://github.com/mst-mkt/nuten/tree/main/tasks/benchmark). The `nix/dev` devShell provides hyperfine, valgrind, and the corpus, and `nix build ./nix/dev#nufmt` builds nufmt. To measure, run the following from Nushell in the devShell.

```nushell
source tasks/benchmark/main.nu
main nufmt $env.NUTEN_CORPUS path/to/nuten path/to/nufmt path/to/nuten-oracle --runs 10 | to json | save -f compare.json
```
