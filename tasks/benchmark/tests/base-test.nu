use std/assert
use std/testing *

source ../base.nu

const BASE = path self ../base.nu

@before-each
def setup []: nothing -> record {
    if $env.NUTEN? == null {
        error make { msg: "NUTEN names no nuten; run the tests with scripts/test-nu.nu" }
    }
    let dir = mktemp --directory
    let corpus = $dir | path join corpus
    mkdir $corpus

    { dir: $dir corpus: $corpus nuten: $env.NUTEN }
}

@after-each
def teardown []: record -> nothing {
    rm --recursive $in.dir
}

def put [corpus: path file: string content: string]: nothing -> path {
    let path = $corpus | path join $file
    mkdir ($path | path dirname)
    $content | save --force $path
    $path
}

def picks []: nothing -> record {
    {
        largest: a.nu
        most_interpolations: b.nu
        most_pipes: a.nu
        largest_in_stdlib: nu-std/std/c.nu
        directory: nu-std
    }
}

def put-picks [corpus: path]: nothing -> nothing {
    picks | reject directory | values | uniq | each {|file| put $corpus $file "ls" } | ignore
}

def input [dir: path source: string]: nothing -> record {
    {
        name: test
        arguments: [format --stdin-filepath ($dir | path join test.nu) -]
        stdin: $source
        exit_codes: [0]
    }
}

@test
def "instructions reads count from cachegrind report" [] {
    let report = "==1== \n==1== I refs:        345,770,384\n"

    let actual = $report | instructions

    assert equal $actual 345770384
}

@test
def "allocations reads blocks and bytes from dhat report" [] {
    let report = [
        "==1== Total:     22,178 bytes in 147 blocks"
        "==1== At t-gmax: 11,274 bytes in 23 blocks"
        "==1== At t-end:  8,736 bytes in 2 blocks"
    ]
    | str join "\n"

    let actual = $report | allocations

    assert equal $actual { allocations: 147 allocated_bytes: 22178 peak_bytes: 11274 }
}

@test
def "nest wraps value in brackets to depth" [] {
    let actual = nest "(" ")" 3

    assert equal $actual "(((1)))"
}

@test
def "file-inputs takes each picked file once" [] {
    let context = $in
    put-picks $context.corpus

    let actual = file-inputs $context.corpus (picks)

    assert equal ($actual | get name) [a.nu b.nu nu-std/std/c.nu]
}

@test
def "file-inputs passes file on stdin with its path" [] {
    let context = $in
    put-picks $context.corpus
    let file = put $context.corpus a.nu "ls | length"

    let actual = file-inputs $context.corpus (picks) | where name == a.nu | first

    assert equal $actual.arguments [format --stdin-filepath $file -]
    assert equal $actual.stdin "ls | length"
}

@test
def "directory-input checks directory in place" [] {
    let context = $in

    let actual = directory-input $context.corpus nu-std

    assert equal $actual.arguments [format --check ($context.corpus | path join nu-std)]
    assert equal $actual.stdin null
}

@test
def "nest-inputs makes nest of each kind of bracket" [] {
    let context = $in

    let actual = nest-inputs $context.corpus

    assert equal ($actual | get name) [nested-parens nested-lists nested-records nested-closures]
}

@test
def "nest-inputs makes nu that nuten formats" [] {
    let context = $in

    let actual = nest-inputs $context.corpus

    for nest in $actual {
        let formatted = $nest.stdin | ^$context.nuten ...$nest.arguments | complete
        assert ($nest.stdin | nu-check) $nest.name
        assert equal $formatted.exit_code 0 $nest.name
    }
}

@test
def "inputs lists files, directory, and nests in turn" [] {
    let context = $in
    put-picks $context.corpus

    let actual = inputs $context.corpus (picks)

    let expected = [
        a.nu
        b.nu
        nu-std/std/c.nu
        nu-std
        nested-parens
        nested-lists
        nested-records
        nested-closures
    ]
    assert equal ($actual | get name) $expected
}

@test
def "measure counts instructions and allocations of nuten" [] {
    let context = $in

    let actual = measure $context.nuten (input $context.dir "ls | length") 100sec

    assert equal $actual.timed_out false
    assert ($actual.instructions > 0)
    assert ($actual.allocations > 0)
    assert ($actual.allocated_bytes >= $actual.peak_bytes)
}

@test
def "measure takes directory nuten would change" [] {
    let context = $in
    put $context.corpus dir/a.nu "ls|length"

    let actual = measure $context.nuten (directory-input $context.corpus dir) 100sec

    assert equal $actual.timed_out false
    assert ($actual.instructions > 0)
}

@test
def "measure gives up on run past time limit" [] {
    let context = $in
    let sleeper = $context.dir | path join sleeper
    "#!/bin/sh\nexec sleep 10\n" | save $sleeper
    ^chmod +x $sleeper

    let actual = measure $sleeper (input $context.dir "") 1sec

    assert equal $actual $TIMED_OUT
}

@test
def "any-allowed-cpu gives cpu taskset runs on" [] {
    let actual = any-allowed-cpu

    let run = ^taskset --cpu-list $actual true | complete
    assert equal $run.exit_code 0 $run.stderr
}

@test
def "measure fails on input nuten rejects" [] {
    let context = $in

    let actual = try {
        measure $context.nuten (input $context.dir "ls | (") 100sec
    } catch {|error|
        $error.msg
    }

    assert str contains $actual "test failed under valgrind"
}

@test
def "main measures every input" [] {
    let context = $in
    open $INPUTS | reject directory | values | uniq | each {|file| put $context.corpus $file "ls" }
    put $context.corpus nu-std/std/lib.nu "ls"

    let actual = main $context.corpus $context.nuten

    assert equal ($actual | get input) (inputs $context.corpus (open $INPUTS) | get name)
    assert equal ($actual | where timed_out | length) 0
}

@test
def "main exits with 2 naming tools not found" [] {
    let context = $in

    let actual = with-env { PATH: [] } {
        ^$nu.current-exe $BASE $context.corpus $context.nuten | complete
    }

    assert equal $actual.exit_code 2
    assert str contains $actual.stderr "valgrind, timeout, taskset not found"
}
