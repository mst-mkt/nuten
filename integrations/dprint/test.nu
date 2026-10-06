use nutest

const SUITE = path self tests/dprint.nu

def main []: nothing -> nothing {
    let plugin = $env.NUTEN_DPRINT? | default --empty { build }

    with-env { NUTEN_DPRINT: $plugin } {
        nutest run-tests --path $SUITE --fail
    }
}

def build []: nothing -> path {
    ^cargo build --release --package nuten-dprint --target wasm32-unknown-unknown --message-format json-render-diagnostics
    | lines
    | each { from json }
    | where reason == compiler-artifact
    | get filenames
    | flatten
    | where $it ends-with .wasm
    | first
    | default --empty { error make { msg: "cargo built no plugin" } }
}
