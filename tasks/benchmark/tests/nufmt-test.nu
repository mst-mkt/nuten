use std/assert
use std/testing *

use ../cpus.nu [allowed-cpus, fastest-allowed-cpu]
source ../nufmt.nu

const NUFMT = path self ../nufmt.nu
const DEV_LOCK = path self ../../../dev/flake.lock

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
            arguments: {|file| [format, --stdin-filepath, $file, -] }
            directory: {|dir| [format, --check, $dir] }
        }
        oracle: $env.NUTEN_ORACLE
    }
}

@after-each
def teardown []: record -> nothing {
    rm --recursive $in.dir
}

def put [dir: path, file: string, content: oneof<string, binary>]: nothing -> path {
    let path = $dir | path join $file
    mkdir ($path | path dirname)
    $content | save --force $path
    $path
}

def pair [formatter: record]: nothing -> list<record> {
    [$formatter, ($formatter | update name nufmt)]
}

def times []: nothing -> list<record> {
    [
        { file: a.nu, nuten: 10ms, nufmt: 40ms }
        { file: b.nu, nuten: 20ms, nufmt: 20ms }
        { file: c.nu, nuten: 20ms, nufmt: 30ms }
    ]
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

# Nu's mktemp reads TMPDIR from the environment the process started with, so time-directories runs in a process of its own.
def time-directories-in-child [
    tmp: path
    file: path
    corpus: path
    formatter: path
]: nothing -> record {
    let script = $"
        source ($NUFMT | to nuon)
        let formatter = { name: nuten command: $env.FORMATTER directory: {|dir| [format --check $dir] } }
        time-directories [$env.FILE] $env.CORPUS [$formatter \($formatter | update name nufmt\)] 1 null
    "
    let environment = { TMPDIR: $tmp, FILE: $file, CORPUS: $corpus, FORMATTER: $formatter }

    with-env $environment { ^$nu.current-exe --no-config-file --commands $script | complete }
}

@test
def "formatters passes nuten path of file and nufmt stdin flag" [] {
    let actual = formatters /bin/nuten /bin/nufmt

    assert equal (do $actual.0.arguments a.nu) [format --stdin-filepath a.nu -]
    assert equal (do $actual.1.arguments a.nu) [--stdin]
}

@test
def "formatters checks directory with nuten and dry-runs it with nufmt" [] {
    let actual = formatters /bin/nuten /bin/nufmt

    assert equal (do $actual.0.directory /d) [format --check /d]
    assert equal (do $actual.1.directory /d) [--dry-run /d]
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
    let splitting = { name: sed, command: sed, arguments: {|file| ['s/|/;/'] } }

    let actual = problem $file "ls | length" $splitting $context.oracle 100sec

    assert equal $actual "changes the AST"
}

@test
def "problem tells formatter that times out" [] {
    let context = $in
    let file = put $context.dir a.nu "ls"
    let sleeping = { name: sleep, command: sleep, arguments: {|file| [10] } }

    let actual = problem $file "ls" $sleeping $context.oracle 1sec

    assert equal $actual "times out"
}

@test
def "problem runs formatter in directory of file" [] {
    let context = $in
    let file = put $context.dir sub/a.nu "ls"
    let telling = { name: sh, command: sh, arguments: {|file| [-c, "pwd > ran-in; cat"] } }

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

@test
def "time-file times each formatter on file" [] {
    let context = $in
    let file = put $context.dir a.nu "ls|length"

    let actual = time-file $file (pair $context.nuten) 1

    assert equal ($actual | columns) [file nuten nufmt]
    assert equal $actual.file $file
    assert ($actual.nuten > 0sec)
    assert ($actual.nufmt > 0sec)
}

@test
def "time-file runs formatter in directory of file" [] {
    let context = $in
    let file = put $context.dir sub/a.nu "ls"
    let telling = { name: sh, command: sh, arguments: {|file| [-c, "pwd > ran-in; cat"] } }

    time-file $file [$telling] 1

    assert (($context.dir | path join sub ran-in) | path exists)
}

@test
def "time-file runs formatter once to warm up and then as many times as asked" [] {
    let context = $in
    let file = put $context.dir a.nu "ls"
    let counting = { name: sh, command: sh, arguments: {|file| [-c, "echo >> ran; cat"] } }

    time-file $file [$counting] 2

    assert equal (open ($context.dir | path join ran) | lines | length) 3
}

@test
def "time-file gives median of runs" [] {
    let context = $in
    let file = put $context.dir a.nu "ls"
    # The three runs after the warmup sleep 0.1, 2, and 0.4 seconds, so only their median lies in [0.4, 0.8).
    let script = "echo >> ran; case $(wc -l < ran) in 2) sleep 0.1 ;; 3) sleep 2 ;; *) sleep 0.4 ;; esac; cat"
    let sleeping = { name: nuten, command: sh, arguments: {|file| [-c, $script] } }

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
    let words = ["/a b/nuten", --stdin-filepath, "/c d/it's.nu"]

    let actual = command-line $words

    assert equal $actual "'/a b/nuten' '--stdin-filepath' '/c d/it'\\''s.nu'"
}

@test
def "staged-path keeps path of file in corpus" [] {
    let actual = staged-path /corpus/a/b.nu /corpus 0

    assert equal $actual a/b.nu
}

@test
def "staged-path puts file from outside corpus in directory of its own" [] {
    let actual = [/elsewhere/a.nu, /corpus-other/a.nu]
        | enumerate
        | each {|entry| staged-path $entry.item /corpus $entry.index }

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

@test
def "time-directory runs formatter on directory from inside it" [] {
    let context = $in
    let dir = $context.dir | path join staged
    mkdir $dir
    let telling = {
        name: sh
        command: sh
        directory: {|dir| [-c, $"pwd > ran-in; echo ($dir) > ran-on"] }
    }

    time-directory $dir [$telling] 1 []

    assert equal (open ($dir | path join ran-in) | str trim) $dir
    assert equal (open ($dir | path join ran-on) | str trim) $dir
}

@test
def "time-directory takes check that finds change as success" [] {
    let context = $in
    let dir = $context.dir | path join staged
    put $dir a.nu "ls|length"

    let actual = time-directory $dir [$context.nuten] 1 []

    assert ($actual.nuten > 0sec)
}

@test
def "time-directory runs formatter behind prefix" [] {
    let context = $in
    let dir = $context.dir | path join staged
    mkdir $dir
    let telling = { name: sh, command: sh, directory: {|dir| [-c, 'echo $PREFIXED > ran'] } }

    time-directory $dir [$telling] 1 [env PREFIXED=yes]

    assert equal (open ($dir | path join ran) | str trim) yes
}

@test
def "time-directory fails naming directory when formatter crashes" [] {
    let context = $in
    let dir = $context.dir | path join staged
    mkdir $dir
    let crashing = { name: sh, command: sh, directory: {|dir| [-c, 'exit 2'] } }

    let actual = try { time-directory $dir [$crashing] 1 [] } catch {|error| $error.msg }

    assert str contains $actual $"hyperfine failed on ($dir)"
}

@test
def "groups gives nu-std and whole staged directory with their files" [] {
    let context = $in
    put $context.dir nu-std/a.nu "ls"
    put $context.dir nu-std/sub/b.nu "ls"
    put $context.dir other/c.nu "ls"

    let actual = groups $context.dir

    assert equal $actual [
        { name: nu-std dir: ($context.dir | path join nu-std) files: 2 }
        { name: all dir: $context.dir files: 3 }
    ]
}

@test
def "groups leaves out group without file" [] {
    let context = $in
    put $context.dir other/a.nu "ls"

    let actual = groups $context.dir

    assert equal ($actual | get name) [all]
}

@test
def "time-group runs formatters on all allowed cpus and on fastest one" [] {
    let context = $in
    let dir = $context.dir | path join staged
    mkdir $dir
    let seen = $context.dir | path join seen
    let telling = {
        name: nuten
        command: sh
        directory: {|dir| [-c, $"grep Cpus_allowed_list /proc/self/status >> ($seen)"] }
    }
    let cpus = { all: (allowed-cpus), one: (fastest-allowed-cpu) }
    let allowed = open /proc/self/status | lines | where $it starts-with Cpus_allowed_list | first

    time-group { name: all dir: $dir files: 0 } (pair $telling) 1 $cpus

    assert equal (open $seen | lines | uniq | sort) (
        [$allowed, $"Cpus_allowed_list:\t($cpus.one)"] | uniq | sort
    )
}

@test
def "time-group gives times, speedups, and gains of both formatters" [] {
    let context = $in
    let dir = $context.dir | path join staged
    put $dir a.nu "ls"
    let cpus = { all: (allowed-cpus), one: (fastest-allowed-cpu) }

    let actual = time-group { name: nu-std dir: $dir files: 1 } (pair $context.nuten) 1 $cpus

    assert equal $actual.name nu-std
    assert equal $actual.files 1
    assert equal ($actual.all_cpus | columns) [nuten nufmt speedup]
    assert equal $actual.one_cpu.cpu $cpus.one
    assert equal $actual.parallel_gain {
        nuten: ($actual.one_cpu.nuten / $actual.all_cpus.nuten)
        nufmt: ($actual.one_cpu.nufmt / $actual.all_cpus.nufmt)
    }
}

@test
def "time-group times nothing on one cpu without cpus to pin to" [] {
    let context = $in
    let dir = $context.dir | path join staged
    put $dir a.nu "ls"

    let actual = time-group { name: all dir: $dir files: 1 } (pair $context.nuten) 1 null

    assert ($actual.all_cpus.nuten > 0sec)
    assert equal $actual.one_cpu null
    assert equal $actual.parallel_gain null
}

@test
def "pin gives taskset on cpus given" [] {
    let actual = pin [1 3]

    assert equal $actual [taskset --cpu-list "1,3"]
}

@test
def "pin gives nothing without cpus" [] {
    let actual = pin null

    assert equal $actual []
}

@test
def "parallel-gain divides time on one cpu by that on all" [] {
    let actual = parallel-gain { nuten: 10ms nufmt: 20ms } { nuten: 30ms nufmt: 20ms }

    assert equal $actual { nuten: 3.0 nufmt: 1.0 }
}

@test
def "parallel-gain gives nothing without time on one cpu" [] {
    let actual = parallel-gain { nuten: 10ms nufmt: 20ms } null

    assert equal $actual null
}

@test
def "time-directories times groups and weighs memory in staged directory" [] {
    let context = $in
    let corpus = $context.dir | path join corpus
    let file = put $corpus nu-std/a.nu "ls"

    let actual = time-directories [$file] $corpus (pair $context.nuten) 1 null

    assert equal ($actual.directories | get name) [nu-std all]
    assert ($actual.memory.nuten > 0b)
}

@test
def "time-directories removes staged directory" [] {
    let context = $in
    let corpus = $context.dir | path join corpus
    let file = put $corpus a.nu "ls"
    let tmp = $context.dir | path join tmp
    mkdir $tmp

    let actual = time-directories-in-child $tmp $file $corpus $context.nuten.command

    assert equal $actual.exit_code 0 $actual.stderr
    assert equal (ls $tmp) []
}

@test
def "time-directories removes staged directory when formatter crashes" [] {
    let context = $in
    let corpus = $context.dir | path join corpus
    let file = put $corpus a.nu "ls"
    let tmp = $context.dir | path join tmp
    mkdir $tmp
    let crashing = put $context.dir crash "#!/bin/sh\nexit 2\n"
    chmod +x $crashing

    let actual = time-directories-in-child $tmp $file $corpus $crashing

    assert str contains $actual.stderr "hyperfine failed"
    assert equal (ls $tmp) []
}

@test
def "max-rss reads kibibytes on last line" [] {
    let report = "Command exited with non-zero status 1\n4532\n"

    let actual = $report | max-rss

    assert equal $actual 4532KiB
}

@test
def "memory gives peak memory of each formatter on directory" [] {
    let context = $in
    let dir = $context.dir | path join staged
    put $dir a.nu "ls|length"

    let actual = memory $dir (pair $context.nuten)

    assert equal ($actual | columns) [nuten nufmt]
    assert ($actual.nuten > 0b)
    assert ($actual.nufmt > 0b)
}

@test
def "memory weighs nothing without gnu time" [] {
    let context = $in

    let actual = with-env { PATH: [] } { memory $context.dir [$context.nuten] }

    assert equal $actual null
}

@test
def "memory fails naming formatter that crashes" [] {
    let context = $in
    let crashing = { name: sh, command: sh, directory: {|dir| [-c, 'exit 2'] } }

    let actual = try {
        memory $context.dir [$crashing]
    } catch {|error|
        $error.rendered | ansi strip
    }

    assert str contains $actual $"sh failed on ($context.dir) with exit code 2"
}

@test
def "memory runs formatter on directory from inside it" [] {
    let context = $in
    let dir = $context.dir | path join staged
    mkdir $dir
    let telling = {
        name: sh
        command: sh
        directory: {|dir| [-c, $"pwd > ran-in; echo ($dir) > ran-on"] }
    }

    memory $dir [$telling]

    assert equal (open ($dir | path join ran-in) | str trim) $dir
    assert equal (open ($dir | path join ran-on) | str trim) $dir
}

@test
def "sizes gives size of each binary" [] {
    let context = $in
    let nuten = put $context.dir nuten "12345"
    let nufmt = put $context.dir nufmt "1234567"
    let formatters = [{ name: nuten, command: $nuten }, { name: nufmt, command: $nufmt }]

    let actual = sizes $formatters

    assert equal $actual { nuten: 5b nufmt: 7b }
}

@test
def "with-speedup divides time of nufmt by that of nuten" [] {
    let actual = { nuten: 10ms, nufmt: 15ms } | with-speedup

    assert equal $actual.speedup 1.5
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
    let values = [5, 1, 4, 2, 3, 10, 9, 8, 7, 6]

    let actual = [10, 50, 100] | each {|percent| $values | percentile $percent }

    assert equal $actual [1 5 10]
}

@test
def "percentile rounds rank up" [] {
    let values = [5, 1, 4, 2, 3, 10, 9, 8, 7, 6]

    let actual = [11, 14] | each {|percent| $values | percentile $percent }

    assert equal $actual [2 2]
}

@test
def "percentile gives smallest value for 0" [] {
    let values = [5, 1, 4, 2, 3, 10, 9, 8, 7, 6]

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
def "versions gives no version for node lock lacks" [] {
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
def "main takes file given twice once" [] {
    let context = $in
    let file = put $context.dir a.nu "ls"

    # nuten stands in for nufmt too, and fails on the flag nufmt takes.
    let actual = main $context.dir $file --nuten $context.nuten.command --nufmt $context.nuten.command --oracle $context.oracle

    assert equal $actual.excluded [{ formatter: nufmt reason: fails files: [$file] }]
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
def "main times no directory when every file is left out" [] {
    let context = $in
    put $context.dir a.nu "ls"

    # nuten stands in for nufmt too, and fails on the flag nufmt takes.
    let actual = main $context.dir --nuten $context.nuten.command --nufmt $context.nuten.command --oracle $context.oracle --runs 1

    assert equal $actual.directories []
}

@test
def "main weighs no memory when every file is left out" [] {
    let context = $in
    put $context.dir a.nu "ls"

    # nuten stands in for nufmt too, and fails on the flag nufmt takes.
    let actual = main $context.dir --nuten $context.nuten.command --nufmt $context.nuten.command --oracle $context.oracle --runs 1

    assert equal $actual.memory null
}

@test
def "main gives size of each binary" [] {
    let context = $in
    put $context.dir a.nu "ls"
    let size = ls $context.nuten.command | get 0.size

    # nuten stands in for nufmt too, and fails on the flag nufmt takes.
    let actual = main $context.dir --nuten $context.nuten.command --nufmt $context.nuten.command --oracle $context.oracle --runs 1

    assert equal $actual.sizes { nuten: $size nufmt: $size }
}

@test
def "main gives versions nufmt and corpus are locked at" [] {
    let context = $in
    put $context.dir a.nu "ls"
    let nodes = open --raw $DEV_LOCK | from json | get nodes

    # nuten stands in for nufmt too, and fails on the flag nufmt takes.
    let actual = main $context.dir --nuten $context.nuten.command --nufmt $context.nuten.command --oracle $context.oracle --runs 1

    assert equal $actual.versions.nufmt.rev $nodes.nufmt.locked.rev
    assert equal $actual.versions.corpus.nupm.rev $nodes.corpus-nupm.locked.rev
}

@test
def "main gives machine it runs on" [] {
    let context = $in
    put $context.dir a.nu "ls"

    # nuten stands in for nufmt too, and fails on the flag nufmt takes.
    let actual = main $context.dir --nuten $context.nuten.command --nufmt $context.nuten.command --oracle $context.oracle --runs 1

    assert equal $actual.environment.cpus (sys cpu | length)
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
