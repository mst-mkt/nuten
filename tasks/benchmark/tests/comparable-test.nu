use std/assert
use std/testing *

use support.nu [nuten-formatter, put]
source ../lib/comparable.nu

@before-each
def setup []: nothing -> record {
    if $env.NUTEN? == null or $env.NUTEN_ORACLE? == null {
        error make { msg: "NUTEN or NUTEN_ORACLE names nothing; run the tests with scripts/test-nu.nu" }
    }
    let dir = mktemp --directory

    {
        dir: $dir
        nuten: (nuten-formatter $env.NUTEN)
        oracle: $env.NUTEN_ORACLE
    }
}

@after-each
def teardown []: record -> nothing {
    rm --recursive $in.dir
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
    let splitting = { name: sed, file: {|file| [sed, 's/|/;/'] } }

    let actual = problem $file "ls | length" $splitting $context.oracle 100sec

    assert equal $actual "changes the AST"
}

@test
def "problem tells formatter that times out" [] {
    let context = $in
    let file = put $context.dir a.nu "ls"
    let sleeping = { name: sleep, file: {|file| [sleep, 10] } }

    let actual = problem $file "ls" $sleeping $context.oracle 1sec

    assert equal $actual "times out"
}

@test
def "problem runs formatter in directory of file" [] {
    let context = $in
    let file = put $context.dir sub/a.nu "ls"
    let telling = { name: sh, file: {|file| [sh, -c, "pwd > ran-in; cat"] } }

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
    let formatters = [$context.nuten, ($context.nuten | update name other)]

    let actual = verdict $file $formatters $context.oracle 100sec

    assert equal $actual.problems [{ formatter: nuten reason: fails } { formatter: other reason: fails }]
}

@test
def "report compares only files every formatter keeps" [] {
    let verdicts = [
        { file: a.nu, unreadable: false, problems: [] }
        { file: b.nu, unreadable: false, problems: [{ formatter: nufmt, reason: fails }] }
        { file: c.nu, unreadable: true, problems: [] }
    ]

    let actual = $verdicts | report

    assert equal $actual.compared [a.nu]
}

@test
def "report lists unreadable files apart" [] {
    let verdicts = [
        { file: a.nu, unreadable: false, problems: [] }
        { file: c.nu, unreadable: true, problems: [] }
    ]

    let actual = $verdicts | report

    assert equal $actual.unreadable [c.nu]
}

@test
def "excluded groups files by formatter and reason" [] {
    let verdicts = [
        { file: a.nu, unreadable: false, problems: [{ formatter: nufmt, reason: fails }] }
        { file: b.nu, unreadable: false, problems: [] }
        { file: c.nu, unreadable: false, problems: [{ formatter: nufmt, reason: fails }] }
    ]

    let actual = $verdicts | excluded

    assert equal $actual [{ formatter: nufmt reason: fails files: [a.nu c.nu] }]
}
