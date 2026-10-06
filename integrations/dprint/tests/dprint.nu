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

    { dir: $dir, plugin: $plugin }
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
def "dprint resolves empty nuten section to defaults of nuten" [] {
    let context = $in

    let actual = run-dprint $context.dir output-resolved-config

    assert equal $actual.exit_code 0 $actual.stderr
    let expected = {
        lineWidth: 100
        useTabs: false
        indentWidth: 4
        newLineKind: auto
        commas: inline
    }
    assert equal ($actual.stdout | from json) { nuten: $expected }
}

@test
def "dprint formats with global settings under nuten section" [] {
    let context = $in
    let settings = {
        lineWidth: 20
        indentWidth: 8
        nuten: { indentWidth: 2, commas: always }
        plugins: [$context.plugin]
    }
    $settings | save --force ($context.dir | path join dprint.json)
    let file = put $context.dir a.nu "[alpha bravo charlie]\n"

    let actual = run-dprint $context.dir fmt a.nu

    assert equal $actual.exit_code 0 $actual.stderr
    assert equal (open --raw $file) "[\n  alpha,\n  bravo,\n  charlie\n]\n"
}

@test
def "dprint reports invalid nuten setting" [] {
    let context = $in
    let settings = { nuten: { commas: x }, plugins: [$context.plugin] }
    $settings | save --force ($context.dir | path join dprint.json)
    put $context.dir a.nu "ls\n" | ignore

    let actual = run-dprint $context.dir check a.nu

    assert not equal $actual.exit_code 0
    assert str contains $actual.stderr "(commas)"
}
