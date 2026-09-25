use std/assert
use std/testing *

source ../nufmt.nu

const NUFMT = path self ../nufmt.nu

@before-each
def setup []: nothing -> record {
    if $env.NUTEN? == null or $env.NUTEN_ORACLE? == null {
        error make { msg: "NUTEN or NUTEN_ORACLE names nothing; run the tests with scripts/test-nu.nu" }
    }
    let dir = mktemp --directory

    {
        dir: $dir
        nuten: {
            name: nuten
            command: $env.NUTEN
            arguments: {|file| [format --stdin-filepath $file -] }
        }
        oracle: $env.NUTEN_ORACLE
    }
}

@after-each
def teardown []: record -> nothing {
    rm --recursive $in.dir
}

def put [dir: path file: string content: oneof<string, binary>]: nothing -> path {
    let path = $dir | path join $file
    mkdir ($path | path dirname)
    $content | save --force $path
    $path
}

@test
def "formatters passes nuten path of file and nufmt stdin flag" [] {
    let actual = formatters /bin/nuten /bin/nufmt

    assert equal (do $actual.0.arguments a.nu) [format --stdin-filepath a.nu -]
    assert equal (do $actual.1.arguments a.nu) [--stdin]
}

@test
def "problem gives nothing for formatter that keeps ast" [] {
    let context = $in
    let file = put $context.dir a.nu "ls|length"

    let actual = problem $file "ls|length" $context.nuten $context.oracle 100sec

    assert equal $actual null
}

@test
def "problem tells formatter that fails" [] {
    let context = $in
    let file = put $context.dir a.nu "ls | ("

    let actual = problem $file "ls | (" $context.nuten $context.oracle 100sec

    assert equal $actual fails
}

@test
def "problem tells formatter that changes ast" [] {
    let context = $in
    let file = put $context.dir a.nu "ls | length"
    let splitting = { name: sed command: sed arguments: {|file| ['s/|/;/'] } }

    let actual = problem $file "ls | length" $splitting $context.oracle 100sec

    assert equal $actual "changes the AST"
}

@test
def "problem tells formatter that times out" [] {
    let context = $in
    let file = put $context.dir a.nu "ls"
    let sleeping = { name: sleep command: sleep arguments: {|file| [10] } }

    let actual = problem $file "ls" $sleeping $context.oracle 1sec

    assert equal $actual "times out"
}

@test
def "problem runs formatter in directory of file" [] {
    let context = $in
    let file = put $context.dir sub/a.nu "ls"
    let telling = { name: sh command: sh arguments: {|file| [-c "pwd > ran-in; cat"] } }

    problem $file "ls" $telling $context.oracle 100sec

    assert (($context.dir | path join sub ran-in) | path exists)
}

@test
def "verdict marks file not in utf-8 unreadable" [] {
    let context = $in
    let file = put $context.dir a.nu (0x[ff fe])

    let actual = verdict $file [$context.nuten] $context.oracle 100sec

    assert equal $actual { file: $file unreadable: true problems: [] }
}

@test
def "verdict lists problem of each formatter" [] {
    let context = $in
    let file = put $context.dir a.nu "ls | ("
    let formatters = [$context.nuten ($context.nuten | update name other)]

    let actual = verdict $file $formatters $context.oracle 100sec

    assert equal $actual.problems [{ formatter: nuten reason: fails } { formatter: other reason: fails }]
}

@test
def "report compares only files every formatter keeps" [] {
    let verdicts = [
        { file: a.nu unreadable: false problems: [] }
        { file: b.nu unreadable: false problems: [{ formatter: nufmt reason: fails }] }
        { file: c.nu unreadable: true problems: [] }
    ]

    let actual = $verdicts | report

    assert equal $actual.compared [a.nu]
}

@test
def "report lists unreadable files apart" [] {
    let verdicts = [
        { file: a.nu unreadable: false problems: [] }
        { file: c.nu unreadable: true problems: [] }
    ]

    let actual = $verdicts | report

    assert equal $actual.unreadable [c.nu]
}

@test
def "excluded groups files by formatter and reason" [] {
    let verdicts = [
        { file: a.nu unreadable: false problems: [{ formatter: nufmt reason: fails }] }
        { file: b.nu unreadable: false problems: [] }
        { file: c.nu unreadable: false problems: [{ formatter: nufmt reason: fails }] }
    ]

    let actual = $verdicts | excluded

    assert equal $actual [{ formatter: nufmt reason: fails files: [a.nu c.nu] }]
}

@test
def "main takes files under corpus and after it" [] {
    let context = $in
    let corpus = $context.dir | path join corpus
    let inside = put $corpus sub/a.nu "ls"
    let after = put $context.dir b.nu "ls"

    # nuten stands in for nufmt too, and fails on the flag nufmt takes.
    let actual = main $corpus $after --nuten $context.nuten.command --nufmt $context.nuten.command --oracle $context.oracle

    assert equal $actual.compared []
    assert equal $actual.excluded [{ formatter: nufmt reason: fails files: [$after $inside] }]
}

@test
def "main takes relative paths from where it runs" [] {
    let context = $in
    let corpus = $context.dir | path join corpus
    mkdir $corpus
    let after = put $context.dir b.nu "ls"

    let actual = do {
        cd $context.dir
        main corpus b.nu --nuten $context.nuten.command --nufmt $context.nuten.command --oracle $context.oracle
    }

    assert equal $actual.excluded [{ formatter: nufmt reason: fails files: [$after] }]
}

@test
def "main exits with 2 naming flags not given" [] {
    let context = $in

    let actual = ^$nu.current-exe $NUFMT $context.dir --nuten $context.nuten.command | complete

    assert equal $actual.exit_code 2
    assert str contains $actual.stderr "--nufmt, --oracle not given"
}
