use nutest

const LINUX_ONLY = [
    tasks/benchmark/** # The benchmark measures with valgrind and taskset, which only Linux has.
]

# Runs the nu tests with NUTEN and NUTEN_ORACLE, or else with those built from the working tree.
def main []: nothing -> nothing {
    let suites = suites
    if ($suites | is-empty) {
        print "no nu tests run on this system"
        return
    }

    let nuten = $env.NUTEN? | default --empty { build nuten }
    let oracle = $env.NUTEN_ORACLE? | default --empty { build nuten-oracle }
    let pattern = $suites | str join '|'

    with-env { NUTEN: $nuten NUTEN_ORACLE: $oracle } {
        nutest run-tests --path . --match-suites $"^\(($pattern)\)$" --fail
    }
}

# Nutest selects suites by file stem only, so the files are picked here.
def suites []: nothing -> list<string> {
    let excluded = if $nu.os-info.name == linux { [] } else { $LINUX_ONLY }

    glob **/*-test.nu --exclude [target/** **/node_modules/** ...$excluded]
    | each { path parse | get stem }
}

# Cargo tells the path, which CARGO_TARGET_DIR may move.
def build [package: string]: nothing -> path {
    ^cargo build --release --package $package --message-format json
    | lines
    | each { from json }
    | where reason == compiler-artifact and executable? != null
    | get executable
    | first
}
