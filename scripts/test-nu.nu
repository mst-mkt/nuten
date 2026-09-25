use nutest

# Runs the nu tests with NUTEN, or else with the nuten built from the working tree.
def main []: nothing -> nothing {
    let nuten = $env.NUTEN? | default --empty { build-nuten }

    with-env { NUTEN: $nuten } { nutest run-tests --path . --match-suites '-test$' --fail }
}

# Cargo tells the path, which CARGO_TARGET_DIR may move.
def build-nuten []: nothing -> path {
    ^cargo build --release --package nuten --message-format json
    | lines
    | each { from json }
    | where reason == compiler-artifact and executable? != null
    | get executable
    | first
}
