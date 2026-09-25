use std/assert
use std/testing *

source ../select.nu

const SELECT = path self ../select.nu

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

def put [corpus: path file: string content: oneof<string, binary>]: nothing -> path {
    let path = $corpus | path join $file
    mkdir ($path | path dirname)
    $content | save $path
    $path
}

@test
def "measure counts bytes in utf-8" [] {
    let actual = "é" | measure

    assert equal $actual.bytes 2
}

@test
def "measure counts pipes wherever nu reads them" [] {
    let cases = [[source pipes];
        ['def f [] { ls | length }' 1]
        ["match 1 {\n    1 => { ls | length }\n    _ => (ls | first | get name)\n}" 3]
        ['{|r| $r | into string | str length }' 2]
        ['$"a (ls | length)"' 1]
        ['module m { export def f [] { [1 2] | each {|x| $x | into string } } }' 2]
        ['if true { ls | length } else { ps | length }' 2]
        ['try { ls | length } catch {|e| $e | get msg }' 2]
        ['for i in 1..2 { $i | into string }' 1]
        ["@example \"x\" { 1 | into string }\ndef f [] { 1 | into string }" 2]
        ['{ a: (1 | into string), b: [(2 | into string)] }' 2]
        ['ls o+e>| length' 1]
    ]

    for case in $cases {
        let actual = $case.source | measure

        assert equal $actual.pipes $case.pipes $case.source
    }
}

@test
def "measure leaves out bars inside strings and comments" [] {
    let sources = [
        "print 'a | b'"
        'print "a | b"'
        "print r#'a | b'#"
        'print 1 # a | b'
    ]

    for source in $sources {
        let actual = $source | measure

        assert equal $actual.pipes 0 $source
    }
}

@test
def "measure counts each interpolated string, nested ones included" [] {
    let cases = [[source interpolations];
        ['$"a (1)"' 1]
        ["$'a (1)'" 1]
        ['$"a ($"b (1)") c"' 2]
    ]

    for case in $cases {
        let actual = $case.source | measure

        assert equal $actual.interpolations $case.interpolations $case.source
    }
}

@test
def "measure leaves out strings that only look interpolated" [] {
    let sources = [
        '"a (1)"'
        "r#'$\"a (1)\"'#"
        '# $"a (1)"'
    ]

    for source in $sources {
        let actual = $source | measure

        assert equal $actual.interpolations 0 $source
    }
}

@test
def "most returns path with highest measure" [] {
    let files = [[path pipes];
        [a.nu 1]
        [b.nu 3]
        [c.nu 2]
    ]

    let actual = $files | most pipes

    assert equal $actual b.nu
}

@test
def "most breaks tie by path order" [] {
    let files = [[path pipes];
        [c.nu 3]
        [a.nu 3]
        [b.nu 3]
    ]

    let actual = $files | most pipes

    assert equal $actual a.nu
}

@test
def "most returns nothing for no files" [] {
    let actual = [] | most pipes

    assert equal $actual null
}

@test
def "pick-inputs picks each input by its rule" [] {
    let files = [[path bytes interpolations pipes];
        [large.nu 50 1 1]
        [interpolating.nu 10 9 1]
        [piping.nu 10 1 9]
        [nu-std/std/lib.nu 20 1 1]
    ]
    let expected = {
        largest: large.nu
        most_interpolations: interpolating.nu
        most_pipes: piping.nu
        largest_in_stdlib: nu-std/std/lib.nu
        directory: nu-std
    }

    let actual = $files | pick-inputs

    assert equal $actual $expected
}

@test
def "pick-inputs leaves rule unpicked when no file meets it" [] {
    let files = [[path bytes interpolations pipes];
        [a.nu 10 1 1]
    ]

    let actual = $files | pick-inputs

    assert equal $actual.largest_in_stdlib null
}

@test
def "candidate takes file nuten formats" [] {
    let context = $in
    let file = put $context.corpus a.nu "ls | length"

    let actual = candidate $file $context.corpus $context.nuten

    assert equal $actual.source "ls | length"
}

@test
def "candidate gives path relative to corpus" [] {
    let context = $in
    let file = put $context.corpus a/b.nu "ls"

    let actual = candidate $file $context.corpus $context.nuten

    assert equal $actual.path a/b.nu
}

@test
def "candidate leaves out file nuten fails on" [] {
    let context = $in
    let file = put $context.corpus a.nu "ls | ("

    let actual = candidate $file $context.corpus $context.nuten

    assert equal $actual null
}

@test
def "candidate leaves out file whose config nuten rejects" [] {
    let context = $in
    put $context.corpus a/nuten.nuon '{ line_width: "broken" }'
    let file = put $context.corpus a/b.nu "ls"

    let actual = candidate $file $context.corpus $context.nuten

    assert equal $actual null
}

@test
def "candidate leaves out file not in utf-8" [] {
    let context = $in
    let file = put $context.corpus a.nu (0x[ff fe])

    let actual = candidate $file $context.corpus $context.nuten

    assert equal $actual null
}

@test
def "render puts header before nuon" [] {
    let context = $in

    let actual = { largest: a.nu } | render $context.nuten

    assert ($actual starts-with $HEADER)
    assert equal ($actual | from nuon) { largest: a.nu }
}

@test
def "main picks from nu files that nuten formats anywhere under corpus" [] {
    let context = $in
    let long_pipeline = 1..100 | each { 'ls' } | str join ' | '
    put $context.corpus large.nu $"# (1..200 | each { 'x' } | str join)"
    put $context.corpus interpolating.nu '$"a (1)"; $"b (2)"'
    put $context.corpus deep/nested/piping.nu "ls | sort | reverse | first"
    put $context.corpus nu-std/std/lib.nu "ls"
    put $context.corpus unformattable.nu $"($long_pipeline) | \("
    put $context.corpus other.txt $long_pipeline
    let expected = {
        largest: large.nu
        most_interpolations: interpolating.nu
        most_pipes: deep/nested/piping.nu
        largest_in_stdlib: nu-std/std/lib.nu
        directory: nu-std
    }

    let actual = ^$nu.current-exe $SELECT $context.corpus $context.nuten | complete

    assert equal $actual.exit_code 0 $actual.stderr
    assert equal ($actual.stdout | from nuon) $expected
}

@test
def "main prints nuon nuten leaves unchanged as inputs.nuon" [] {
    let context = $in
    put $context.corpus a.nu "ls"
    put $context.corpus nu-std/std/lib.nu "ls"

    let actual = ^$nu.current-exe $SELECT $context.corpus $context.nuten | complete

    let check = $actual.stdout
    | ^$context.nuten format --check --stdin-filepath $OUTPUT -
    | complete
    assert equal $check.exit_code 0 $check.stderr
}

@test
def "main exits with 2 when corpus holds no nu file" [] {
    let context = $in

    let actual = ^$nu.current-exe $SELECT $context.corpus $context.nuten | complete

    assert equal $actual.exit_code 2
    assert str contains $actual.stderr "no .nu file"
}

@test
def "main exits with 2 naming rule no file meets" [] {
    let context = $in
    put $context.corpus a.nu "ls"

    let actual = ^$nu.current-exe $SELECT $context.corpus $context.nuten | complete

    assert equal $actual.exit_code 2
    assert str contains $actual.stderr "largest_in_stdlib"
}
