use std/assert
use std/testing *

use support.nu put
source ../lib/provenance.nu

@before-each
def setup []: nothing -> record {
    { dir: (mktemp --directory) }
}

@after-each
def teardown []: record -> nothing {
    rm --recursive $in.dir
}

def commit [dir: path]: nothing -> string {
    let config = [
        user.name=test
        user.email=test@example.com
        commit.gpgsign=false
        core.hooksPath=/dev/null
    ]
    let git = [-C, $dir, ...($config | each { [-c $in] } | flatten)]
    ^git ...$git init --quiet
    ^git ...$git add --all
    ^git ...$git commit --quiet -m test
    ^git -C $dir rev-parse --short HEAD | str trim
}

@test
def "versions reads nufmt, nixpkgs-nushell, and corpus from lock" [] {
    let context = $in
    let node = {|rev| { locked: { rev: $rev, lastModified: 1700000000 } } }
    let lock = {
        nodes: {
            nufmt: (do $node a)
            nixpkgs-nushell: (do $node b)
            corpus-nupm: (do $node c)
            fenix: (do $node d)
        }
    }
    let path = put $context.dir flake.lock ($lock | to json)
    let date = 2023-11-14T22:13:20+00:00

    let actual = versions $context.dir $path

    assert equal $actual.nufmt { rev: a date: $date }
    assert equal $actual.nixpkgs-nushell { rev: b date: $date }
    assert equal $actual.corpus { nupm: { rev: c date: $date } }
}

@test
def "versions gives nothing for node missing from lock" [] {
    let context = $in
    let path = put $context.dir flake.lock ({ nodes: {} } | to json)

    let actual = versions $context.dir $path

    assert equal $actual.nufmt null
    assert equal $actual.nixpkgs-nushell null
}

@test
def "versions gives no locked version without lock" [] {
    let context = $in

    let actual = versions $context.dir ($context.dir | path join flake.lock)

    assert equal ($actual | reject nuten) { nufmt: null nixpkgs-nushell: null corpus: null }
}

@test
def "versions names nuten by commit of tree it is in" [] {
    let context = $in
    put $context.dir a.nu "ls"
    let commit = commit $context.dir

    let actual = versions $context.dir ($context.dir | path join flake.lock)

    assert equal $actual.nuten $commit
}

@test
def "versions marks nuten dirty when tree has changes" [] {
    let context = $in
    let file = put $context.dir a.nu "ls"
    let commit = commit $context.dir
    "ls | length" | save --force $file

    let actual = versions $context.dir ($context.dir | path join flake.lock)

    assert equal $actual.nuten $"($commit)-dirty"
}

@test
def "versions gives no nuten version outside git tree" [] {
    let context = $in

    let actual = with-env { GIT_CEILING_DIRECTORIES: ($context.dir | path dirname) } {
        versions $context.dir ($context.dir | path join flake.lock)
    }

    assert equal $actual.nuten null
}

@test
def "environment gives machine without host name" [] {
    let actual = environment

    assert equal ($actual | columns) [os kernel cpu cpus]
    assert equal $actual.kernel (^uname -r | str trim)
    assert ($actual.cpus > 0)
}
