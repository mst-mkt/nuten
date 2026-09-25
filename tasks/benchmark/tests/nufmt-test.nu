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

def times []: nothing -> list<record> {
    [
        { file: a.nu nuten: 10ms nufmt: 40ms }
        { file: b.nu nuten: 20ms nufmt: 20ms }
        { file: c.nu nuten: 20ms nufmt: 30ms }
    ]
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
def "time-file times each formatter on file" [] {
    let context = $in
    let file = put $context.dir a.nu "ls|length"
    let formatters = [$context.nuten ($context.nuten | update name nufmt)]

    let actual = time-file $file $formatters 1

    assert equal ($actual | columns) [file nuten nufmt]
    assert equal $actual.file $file
    assert ($actual.nuten > 0sec)
    assert ($actual.nufmt > 0sec)
}

@test
def "time-file runs formatter in directory of file" [] {
    let context = $in
    let file = put $context.dir sub/a.nu "ls"
    let telling = { name: sh command: sh arguments: {|file| [-c "pwd > ran-in; cat"] } }

    time-file $file [$telling] 1

    assert (($context.dir | path join sub ran-in) | path exists)
}

@test
def "time-file runs formatter once to warm up and then as many times as asked" [] {
    let context = $in
    let file = put $context.dir a.nu "ls"
    let counting = { name: sh command: sh arguments: {|file| [-c "echo >> ran; cat"] } }

    time-file $file [$counting] 2

    assert equal (open ($context.dir | path join ran) | lines | length) 3
}

@test
def "time-file gives median of runs" [] {
    let context = $in
    let file = put $context.dir a.nu "ls"
    # The three runs after the warmup sleep 0.1, 2, and 0.4 seconds, so only their median lies in [0.4, 0.8).
    let script = "echo >> ran; case $(wc -l < ran) in 2) sleep 0.1 ;; 3) sleep 2 ;; *) sleep 0.4 ;; esac; cat"
    let sleeping = { name: nuten command: sh arguments: {|file| [-c $script] } }

    let actual = time-file $file [$sleeping] 3

    assert ($actual.nuten >= 400ms)
    assert ($actual.nuten < 800ms)
}

@test
def "time-file times file whose path holds single quote" [] {
    let context = $in
    let file = put $context.dir "it's.nu" "ls"

    let actual = time-file $file [$context.nuten] 1

    assert ($actual.nuten > 0sec)
}

@test
def "time-file fails naming file when formatter fails" [] {
    let context = $in
    let file = put $context.dir a.nu "ls | ("

    let actual = try { time-file $file [$context.nuten] 1 } catch {|error| $error.msg }

    assert str contains $actual $"hyperfine failed on ($file)"
}

@test
def "command-line quotes each word" [] {
    let formatter = {
        name: nuten
        command: "/a b/nuten"
        arguments: {|file| [--stdin-filepath $file] }
    }

    let actual = command-line $formatter "/c d/it's.nu"

    assert equal $actual "'/a b/nuten' '--stdin-filepath' '/c d/it'\\''s.nu'"
}

@test
def "summarize gives speedup of nuten on each file" [] {
    let actual = times | summarize

    assert equal ($actual.files | select file speedup) [
        { file: a.nu speedup: 4.0 }
        { file: b.nu speedup: 1.0 }
        { file: c.nu speedup: 1.5 }
    ]
}

@test
def "summarize keeps time of each formatter on each file" [] {
    let actual = times | summarize

    assert equal ($actual.files | select file nuten nufmt) (times)
}

@test
def "summarize totals time of each formatter" [] {
    let actual = times | summarize

    assert equal $actual.total { nuten: 50ms nufmt: 90ms }
}

@test
def "summarize gives median speedup of nuten" [] {
    let actual = times | summarize

    assert equal $actual.speedup.median 1.5
}

@test
def "summarize gives 10th percentile speedup of nuten" [] {
    let spread = 1..20 | each {|i| { file: $"($i).nu" nuten: 10ms nufmt: ($i * 10ms) } }

    let actual = $spread | summarize

    assert equal $actual.speedup.p10 2.0
}

@test
def "summarize gives no speedup for no files" [] {
    let actual = [] | summarize

    assert equal $actual { files: [] total: null speedup: null }
}

@test
def "percentile gives value at nearest rank" [] {
    let values = [5 1 4 2 3 10 9 8 7 6]

    let actual = [10 50 100] | each {|percent| $values | percentile $percent }

    assert equal $actual [1 5 10]
}

@test
def "percentile rounds rank up" [] {
    let values = [5 1 4 2 3 10 9 8 7 6]

    let actual = [11 14] | each {|percent| $values | percentile $percent }

    assert equal $actual [2 2]
}

@test
def "percentile gives smallest value for 0" [] {
    let values = [5 1 4 2 3 10 9 8 7 6]

    let actual = $values | percentile 0

    assert equal $actual 1
}

@test
def "percentile does not round whole rank up" [] {
    let values = 1..100 | each { $in }

    let actual = $values | percentile 7

    assert equal $actual 7
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

    # nuten stands in for nufmt too, and fails on the flag nufmt takes.
    let actual = do {
        cd $context.dir
        main corpus b.nu --nuten $context.nuten.command --nufmt $context.nuten.command --oracle $context.oracle
    }

    assert equal $actual.excluded [{ formatter: nufmt reason: fails files: [$after] }]
}

@test
def "main times no file when every file is left out" [] {
    let context = $in
    put $context.dir a.nu "ls"

    # nuten stands in for nufmt too, and fails on the flag nufmt takes.
    let actual = main $context.dir --nuten $context.nuten.command --nufmt $context.nuten.command --oracle $context.oracle --runs 1

    assert equal $actual.timing { files: [] total: null speedup: null }
}

@test
def "main exits with 2 naming flags not given" [] {
    let context = $in

    let actual = ^$nu.current-exe $NUFMT $context.dir --nuten $context.nuten.command | complete

    assert equal $actual.exit_code 2
    assert str contains $actual.stderr "--nufmt, --oracle not given"
}

@test
def "main exits with 2 when asked for no run" [] {
    let context = $in

    let actual = ^$nu.current-exe $NUFMT $context.dir --nuten a --nufmt b --oracle c --runs 0
    | complete

    assert equal $actual.exit_code 2
    assert str contains $actual.stderr "--runs must be at least 1"
}

@test
def "main exits with 2 naming tools not found" [] {
    let context = $in

    let actual = with-env { PATH: [] } {
        ^$nu.current-exe $NUFMT $context.dir --nuten a --nufmt b --oracle c | complete
    }

    assert equal $actual.exit_code 2
    assert str contains $actual.stderr "timeout, hyperfine not found"
}
