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
    let file = put $context.dir a.nu "ls"
    let failing = { name: sh, file: {|file| [sh, -c, "exit 1"] } }

    let actual = problem $file "ls" $failing $context.oracle 100sec

    assert equal $actual fails
}

@test
def "problem tells formatter that rejects file nu rejects" [] {
    let context = $in
    let file = put $context.dir a.nu "ls | ("

    let actual = problem $file "ls | (" $context.nuten $context.oracle 100sec

    assert equal $actual "rejects a file nu rejects"
}

@test
def "problem tells formatter that rejects file nu accepts as failing" [] {
    let context = $in
    let file = put $context.dir a.nu "ls"
    let rejecting = { name: sh, file: {|file| [sh, -c, "exit 2"] }, rejection_exit_code: 2 }

    let actual = problem $file "ls" $rejecting $context.oracle 100sec

    assert equal $actual fails
}

@test
def "problem tells formatter whose exit code is not its rejection code as failing" [] {
    let context = $in
    let file = put $context.dir a.nu "ls | ("
    let failing = { name: sh, file: {|file| [sh, -c, "exit 1"] }, rejection_exit_code: 2 }

    let actual = problem $file "ls | (" $failing $context.oracle 100sec

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
def "problem gives nothing for formatter that changes ast when told to skip ast" [] {
    let context = $in
    let file = put $context.dir a.nu "ls | length"
    let splitting = { name: sed, file: {|file| [sed, 's/|/;/'] } }

    let actual = problem $file "ls | length" $splitting $context.oracle 100sec --skip-ast

    assert equal $actual null
}

@test
def "problem tells formatter that fails when told to skip ast" [] {
    let context = $in
    let file = put $context.dir a.nu "ls"
    let failing = { name: sh, file: {|file| [sh, -c, "exit 1"] } }

    let actual = problem $file "ls" $failing $context.oracle 100sec --skip-ast

    assert equal $actual fails
}

@test
def "problem tells formatter that times out when told to skip ast" [] {
    let context = $in
    let file = put $context.dir a.nu "ls"
    let sleeping = { name: sleep, file: {|file| [sleep, 10] } }

    let actual = problem $file "ls" $sleeping $context.oracle 1sec --skip-ast

    assert equal $actual "times out"
}

@test
def "problem tells formatter that rejects file nu rejects when told to skip ast" [] {
    let context = $in
    let file = put $context.dir a.nu "ls | ("

    let actual = problem $file "ls | (" $context.nuten $context.oracle 100sec --skip-ast

    assert equal $actual "rejects a file nu rejects"
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

    let actual = verdict $file [$context.nuten] $context.oracle 100sec 512KiB

    assert equal $actual { file: $file unreadable: true unverified: false problems: [] }
}

@test
def "verdict lists problem of each formatter" [] {
    let context = $in
    let file = put $context.dir a.nu "ls | ("
    let formatters = [
        $context.nuten
        ($context.nuten | reject rejection_exit_code | update name other)
    ]

    let actual = verdict $file $formatters $context.oracle 100sec 512KiB

    assert equal $actual.problems [
        { formatter: nuten reason: "rejects a file nu rejects" }
        { formatter: other reason: fails }
    ]
}

@test
def "verdict marks file larger than max size unverified and checks no ast" [] {
    let context = $in
    let file = put $context.dir a.nu "ls | length"
    let splitting = { name: sed, file: {|file| [sed, 's/|/;/'] } }

    let actual = verdict $file [$splitting] $context.oracle 100sec 10b

    assert equal $actual { file: $file unreadable: false unverified: true problems: [] }
}

@test
def "verdict checks ast of file of max size" [] {
    let context = $in
    let file = put $context.dir a.nu "ls | length"
    let splitting = { name: sed, file: {|file| [sed, 's/|/;/'] } }

    let actual = verdict $file [$splitting] $context.oracle 100sec 11b

    assert equal $actual {
        file: $file
        unreadable: false
        unverified: false
        problems: [{ formatter: sed reason: "changes the AST" }]
    }
}

@test
def "verdict measures max size in bytes" [] {
    let context = $in
    let file = put $context.dir a.nu "'あ'"
    let keeping = { name: cat, file: {|file| [cat] } }

    let actual = verdict $file [$keeping] $context.oracle 100sec 4b

    assert equal $actual.unverified true
}

@test
def "report compares only files every formatter keeps" [] {
    let verdicts = [
        { file: a.nu, unreadable: false, unverified: false, problems: [] }
        {
            file: b.nu
            unreadable: false
            unverified: false
            problems: [{ formatter: nufmt, reason: fails }]
        }
        { file: c.nu, unreadable: true, unverified: false, problems: [] }
        { file: d.nu, unreadable: false, unverified: true, problems: [] }
    ]

    let actual = $verdicts | report

    assert equal $actual.compared [a.nu d.nu]
}

@test
def "report lists unreadable files apart" [] {
    let verdicts = [
        { file: a.nu, unreadable: false, unverified: false, problems: [] }
        { file: c.nu, unreadable: true, unverified: false, problems: [] }
    ]

    let actual = $verdicts | report

    assert equal $actual.unreadable [c.nu]
}

@test
def "report lists unverified files apart, left out or not" [] {
    let verdicts = [
        { file: a.nu, unreadable: false, unverified: false, problems: [] }
        {
            file: b.nu
            unreadable: false
            unverified: true
            problems: [{ formatter: nufmt, reason: fails }]
        }
        { file: d.nu, unreadable: false, unverified: true, problems: [] }
    ]

    let actual = $verdicts | report

    assert equal $actual.unverified [b.nu d.nu]
}

@test
def "excluded groups files by formatter and reason" [] {
    let verdicts = [
        {
            file: a.nu
            unreadable: false
            unverified: false
            problems: [{ formatter: nufmt, reason: fails }]
        }
        { file: b.nu, unreadable: false, unverified: false, problems: [] }
        {
            file: c.nu
            unreadable: false
            unverified: false
            problems: [{ formatter: nufmt, reason: fails }]
        }
    ]

    let actual = $verdicts | excluded

    assert equal $actual [{ formatter: nufmt reason: fails files: [a.nu c.nu] }]
}
