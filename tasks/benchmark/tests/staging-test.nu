use std/assert
use std/testing *

use support.nu put
source ../staging.nu

@before-each
def setup []: nothing -> record {
    { dir: (mktemp --directory) }
}

@after-each
def teardown []: record -> nothing {
    rm --recursive $in.dir
}

@test
def "staged-path keeps path of file in corpus" [] {
    let actual = staged-path /corpus/a/b.nu /corpus 0

    assert equal $actual a/b.nu
}

@test
def "staged-path puts file from outside corpus in directory of its own" [] {
    let files = [/elsewhere/a.nu, /corpus-other/a.nu]

    let actual = $files | enumerate | each {|entry| staged-path $entry.item /corpus $entry.index }

    assert equal $actual [also/0/a.nu also/1/a.nu]
}

@test
def "staged-path drops leading dot from each part" [] {
    let actual = staged-path /corpus/.github/.hidden.nu /corpus 0

    assert equal $actual _github/_hidden.nu
}

@test
def "stage copies each file to its staged path" [] {
    let context = $in
    let corpus = $context.dir | path join corpus
    let inside = put $corpus .dir/a.nu "ls"
    let outside = put $context.dir b.nu "ls | length"

    let actual = stage [$inside $outside] $corpus

    assert equal (open --raw ($actual | path join _dir/a.nu)) "ls"
    assert equal (open --raw ($actual | path join also/1/b.nu)) "ls | length"
    rm --recursive $actual
}
