use std/assert
use std/testing *

@before-each
def setup []: nothing -> record {
    if $env.NUTEN_DPRINT? == null {
        error make { msg: "NUTEN_DPRINT names no plugin; run the tests with integrations/dprint/test.nu" }
    }
    let dir = mktemp --directory
    let plugin = $env.NUTEN_DPRINT | path expand
    { nuten: {}, plugins: [$plugin] } | save ($dir | path join dprint.json)

    { dir: $dir }
}

@after-each
def teardown []: record -> nothing {
    rm --recursive $in.dir
}

def put [dir: path, file: string, content: string]: nothing -> path {
    let path = $dir | path join $file
    $content | save $path
    $path
}

# The config named, since DPRINT_CONFIG_DISCOVERY and the like can send dprint to another one.
def run-dprint [dir: path, ...arguments: string]: nothing -> record {
    cd $dir
    with-env { DPRINT_CACHE_DIR: ($dir | path join cache) } {
        ^dprint ...$arguments --config dprint.json | complete
    }
}

@test
def "dprint formats nu file" [] {
    let context = $in
    let file = put $context.dir a.nu "ls|sort\n"

    let actual = run-dprint $context.dir fmt a.nu

    assert equal $actual.exit_code 0 $actual.stderr
    assert equal (open --raw $file) "ls | sort\n"
}

@test
def "dprint check passes formatted nuon file" [] {
    let context = $in
    put $context.dir a.nuon "{ a: 1 }\n" | ignore

    let actual = run-dprint $context.dir check a.nuon

    assert equal $actual.exit_code 0 $actual.stderr
}

@test
def "dprint reports parse error at its line and column" [] {
    let context = $in
    put $context.dir a.nu "ls\nfoo && bar\n" | ignore

    let actual = run-dprint $context.dir check a.nu

    assert equal $actual.exit_code 1 $actual.stderr
    assert str contains $actual.stderr ": 2:5: "
}

@test
def "dprint resolves empty nuten section to empty object" [] {
    let context = $in

    let actual = run-dprint $context.dir output-resolved-config

    assert equal $actual.exit_code 0 $actual.stderr
    assert equal ($actual.stdout | from json) { nuten: {} }
}
