# benchmark

The benchmarks of [Nuten](https://github.com/mst-mkt/nuten), a formatter for Nushell.
The `nix/dev` devShell has the tools they need, and sets `NUTEN_CORPUS` to the corpus.

```nushell
nu tasks/benchmark/main.nu --help
```

## Subcommands

### base

Compares two `nuten` builds under valgrind, and CI posts the result on pull requests.

```nushell
nu tasks/benchmark/main.nu base $env.NUTEN_CORPUS path/to/base/nuten path/to/head/nuten
```

| input                                       | metric       | base      | head      | change_percent | regressed |
| ------------------------------------------- | ------------ | --------- | --------- | -------------- | --------- |
| kurokirasama-nushell_scripts/update_apps.nu | instructions | 139485238 | 139486166 | 0.0            | false     |
| kurokirasama-nushell_scripts/update_apps.nu | allocations  | 226145    | 226145    | 0.0            | false     |
| …                                           | …            | …         | …         | …              | …         |

### nufmt

Compares `nuten` with nufmt under hyperfine, on the files both format without breaking.

```nushell
nu tasks/benchmark/main.nu nufmt $env.NUTEN_CORPUS path/to/nuten path/to/nufmt path/to/nuten-oracle
```

```
{
    versions: { nuten, nufmt, nixpkgs-nushell, corpus }
    environment: { os, kernel, cpu, cpus }
    compared: [file]
    excluded: [{ formatter, reason, files }]
    unreadable: [file]
    timing: {
        files: [{ file, time_wall_clock, memory_peak_resident, instructions }]
        total: { time_wall_clock, instructions }
        speedup: { time_wall_clock: { median, p10 }, instructions: { median, p10 } }
    }
    directories: [{ name, files, all_cpus, one_cpu, parallel_gain }]
    sizes: { nuten, nufmt }
}
```

### select

Picks the inputs of `base` from the corpus.

```nushell
nu tasks/benchmark/main.nu select $env.NUTEN_CORPUS path/to/nuten | save -f tasks/benchmark/inputs.nuon
```

```nushell
{
    largest: "nu_scripts/custom-completions/auto-generate/completions/mvn.nu"
    most_interpolations: "kurokirasama-nushell_scripts/update_apps.nu"
    most_pipes: "nu_scripts/sourced/nu_rust_ast/rust_ast.nu"
    largest_in_stdlib: "nu-std/std/help/mod.nu"
    directory: nu-std
}
```

## Tests

`vp run test:nu` runs the [nutest](https://github.com/vyadh/nutest) suites in `tests`, on Linux only.
