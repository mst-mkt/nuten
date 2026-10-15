const OUT_DIR = path self ts/gen

def main []: nothing -> nothing {
    let wasm = build
    ^wasm-bindgen --target web --out-dir $OUT_DIR --out-name nuten $wasm

    rm ($OUT_DIR | path join nuten_bg.wasm.d.ts)
}

def build []: nothing -> path {
    ^cargo build --profile wasm --package nuten-npm-format --target wasm32-unknown-unknown --message-format json-render-diagnostics
    | from json --objects
    | where reason == compiler-artifact
    | get filenames
    | flatten
    | where $it ends-with .wasm
    | first
    | default --empty { error make { msg: "cargo built no wasm" } }
}
