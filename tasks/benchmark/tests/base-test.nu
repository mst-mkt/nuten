use std/assert
use std/testing *

use support.nu put
source ../lib/base.nu

const MAIN = path self ../main.nu

const PICKS = {
    largest: a.nu
    most_interpolations: b.nu
    most_pipes: a.nu
    largest_in_stdlib: nu-std/std/c.nu
    directory: nu-std
}

@before-each
def setup []: nothing -> record {
    if $env.NUTEN? == null {
        error make { msg: "NUTEN names no nuten; run the tests with scripts/test-nu.nu" }
    }
    let dir = mktemp --directory
    let corpus = $dir | path join corpus
    mkdir $corpus

    { dir: $dir, corpus: $corpus, nuten: $env.NUTEN }
}

@after-each
def teardown []: record -> nothing {
    rm --recursive $in.dir
}

def put-picks [corpus: path, picks: record]: nothing -> nothing {
    $picks | reject directory | values | each {|file| put $corpus $file "ls" } | ignore
    put $corpus ($picks.directory | path join lib.nu) "ls" | ignore
}

def sleeper [dir: path]: nothing -> path {
    let path = $dir | path join sleeper
    "#!/bin/sh\nexec sleep 10\n" | save $path
    ^chmod +x $path
    $path
}

def measures [instructions: int]: nothing -> record {
    {
        input: a.nu
        timed_out: false
        instructions: $instructions
        allocations: 10
        allocated_bytes: 100
        peak_bytes: 50
    }
}

def input [dir: path, source: string]: nothing -> record {
    {
        name: test
        arguments: [format, --stdin-filepath, ($dir | path join test.nu), -]
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
    put-picks $context.corpus $PICKS

    let actual = file-inputs $context.corpus $PICKS

    assert equal ($actual | get name) [a.nu b.nu nu-std/std/c.nu]
}

@test
def "file-inputs passes file on stdin with its path" [] {
    let context = $in
    put-picks $context.corpus $PICKS
    let file = put $context.corpus a.nu "ls | length"

    let actual = file-inputs $context.corpus $PICKS | where name == a.nu | first

    assert equal $actual.arguments [format --stdin-filepath $file -]
    assert equal $actual.stdin "ls | length"
}

@test
def "directory-input checks directory in place" [] {
    let context = $in

    let actual = directory-input $context.corpus nu-std

    assert equal $actual.arguments [format --list-different ($context.corpus | path join nu-std)]
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
    put-picks $context.corpus $PICKS
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

    let actual = inputs $context.corpus $PICKS

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

    let actual = measure (sleeper $context.dir) (input $context.dir "") 1sec

    assert equal $actual $TIMED_OUT
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
def "compare gives row for each metric" [] {
    let actual = compare [(measures 1000)] [(measures 1000)]

    assert equal ($actual | get metric) $METRICS
}

@test
def "compare finds no regression in same measures" [] {
    let actual = compare [(measures 1000)] [(measures 1000)]

    assert equal ($actual | where regressed | length) 0
}

@test
def "compare gives change in percent" [] {
    let actual = compare [(measures 1000)] [(measures 1234)]

    let instructions = $actual | where metric == instructions | first
    assert equal $instructions.change_percent 23.4
}

@test
def "compare marks metric grown past threshold" [] {
    let actual = compare [(measures 1000)] [(measures 1011)]

    assert equal ($actual | where regressed | get metric) [instructions]
}

@test
def "compare lets metric grow within threshold" [] {
    let actual = compare [(measures 1000)] [(measures 1010)]

    assert equal ($actual | where regressed | length) 0
}

@test
def "compare takes shrink as no regression" [] {
    let actual = compare [(measures 1000)] [(measures 500)]

    assert equal ($actual | where regressed | length) 0
}

@test
def "compare marks input head timed out on" [] {
    let actual = compare [(measures 1000)] [{ input: a.nu ...$TIMED_OUT }]

    let rows = $actual | select input metric regressed
    assert equal $rows [{ input: a.nu metric: timed_out regressed: true }]
}

@test
def "changes finds no regression when head does same work as base" [] {
    let context = $in
    put-picks $context.corpus (open $INPUTS)
    let names = inputs $context.corpus (open $INPUTS) | get name

    let actual = changes $context.corpus $context.nuten $context.nuten 100sec | get changes

    assert equal ($actual | get input | uniq) $names
    assert equal ($actual | length) (($names | length) * ($METRICS | length))
    assert equal ($actual | where regressed | length) 0
}

@test
def "main base exits with 2 naming inputs base times out on" [] {
    let context = $in
    put-picks $context.corpus (open $INPUTS)
    let base = sleeper $context.dir

    let actual = ^$nu.current-exe $MAIN base --time-limit 1sec $context.corpus $base $context.nuten
        | complete

    assert equal $actual.exit_code 2
    assert str contains $actual.stderr "base timed out on"
}

@test
def "main base exits with 2 naming tools not found" [] {
    let context = $in

    let actual = with-env { PATH: [] } {
        ^$nu.current-exe $MAIN base $context.corpus $context.nuten $context.nuten | complete
    }

    assert equal $actual.exit_code 2
    assert str contains $actual.stderr "valgrind, timeout, taskset not found"
}
