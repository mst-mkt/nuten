use nutest

# Runs the nu tests with NUTEN and NUTEN_ORACLE, or else with those built from the working tree.
def main []: nothing -> nothing {
    let nuten = $env.NUTEN? | default --empty { build nuten }
    let oracle = $env.NUTEN_ORACLE? | default --empty { build nuten-oracle }

    with-env { NUTEN: $nuten NUTEN_ORACLE: $oracle } {
        nutest run-tests --path . --match-suites '-test$' --fail
    }
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
