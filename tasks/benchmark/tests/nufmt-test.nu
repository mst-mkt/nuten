use std/assert
use std/testing *

use ../lib/cpus.nu [allowed-cpus, fastest-allowed-cpu]
use support.nu [nuten-formatter, put]
source ../lib/nufmt.nu

const MAIN = path self ../main.nu
const NUFMT = path self ../lib/nufmt.nu

@before-each
def setup []: nothing -> record {
    if $env.NUTEN? == null or $env.NUTEN_ORACLE? == null {
        error make { msg: "NUTEN or NUTEN_ORACLE names nothing; run the tests with scripts/test-nu.nu" }
    }
    let dir = mktemp --directory

    {
        dir: $dir
        nuten: (nuten-formatter $env.NUTEN)
        # nuten stands in for nufmt too, and fails on the flag nufmt takes.
        failing_nufmt: $env.NUTEN
        oracle: $env.NUTEN_ORACLE
    }
}

@after-each
def teardown []: record -> nothing {
    rm --recursive $in.dir
}

# Each file passes through unchanged, so the oracle finds the same AST.
def keeping-nufmt [dir: path]: nothing -> path {
    let nufmt = put $dir nufmt "#!/bin/sh\ncase \"$1\" in --stdin) cat ;; esac\n"
    chmod +x $nufmt
    $nufmt
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

# Nu's mktemp reads TMPDIR from the environment the process started with, so time-directories runs in a process of its own.
def time-directories-in-child [
    tmp: path
    file: path
    corpus: path
    formatter: path
]: nothing -> record {
    let script = $"
        source ($NUFMT | to nuon)
        let formatter = { name: nuten directory: {|dir| [$env.FORMATTER format --list-different $dir] } }
        time-directories [$env.FILE] $env.CORPUS [$formatter \($formatter | update name nufmt\)] 1 null
    "
    let environment = { TMPDIR: $tmp, FILE: $file, CORPUS: $corpus, FORMATTER: $formatter }

    with-env $environment { ^$nu.current-exe --no-config-file --commands $script | complete }
}

@test
def "formatters gives nuten file path and nufmt stdin flag" [] {
    let actual = formatters /bin/nuten /bin/nufmt

    assert equal (do $actual.0.file a.nu) [/bin/nuten format --stdin-filepath a.nu -]
    assert equal (do $actual.1.file a.nu) [/bin/nufmt --stdin]
}

@test
def "formatters checks directory with nuten and dry-runs it with nufmt" [] {
    let actual = formatters /bin/nuten /bin/nufmt

    assert equal (do $actual.0.directory /d) [/bin/nuten format --list-different /d]
    assert equal (do $actual.1.directory /d) [/bin/nufmt --dry-run /d]
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
def "time-file runs formatter in directory of file with file on stdin" [] {
    let context = $in
    let file = put $context.dir sub/a.nu "ls"
    let telling = { name: sh, file: {|file| [sh, -c, "pwd > ran-in; cat > got"] } }

    time-file $file [$telling] 1

    assert (($context.dir | path join sub ran-in) | path exists)
    assert equal (open --raw ($context.dir | path join sub got)) "ls"
}

@test
def "time-file times file whose path holds single quote" [] {
    let context = $in
    let file = put $context.dir "it's.nu" "ls"

    let actual = time-file $file [$context.nuten] 1

    assert ($actual.nuten > 0sec)
}

@test
def "time-directory runs formatter on directory from inside it" [] {
    let context = $in
    let dir = $context.dir | path join staged
    mkdir $dir
    let telling = {
        name: sh
        directory: {|dir| [sh, -c, $"pwd > ran-in; echo ($dir) > ran-on"] }
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

    assert ($actual.time_wall_clock.nuten > 0sec)
}

@test
def "time-directory runs formatter behind prefix" [] {
    let context = $in
    let dir = $context.dir | path join staged
    mkdir $dir
    let telling = { name: sh, directory: {|dir| [sh, -c, 'echo $PREFIXED > ran'] } }

    time-directory $dir [$telling] 1 [env PREFIXED=yes]

    assert equal (open ($dir | path join ran) | str trim) yes
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
        directory: {|dir| [sh, -c, $"grep Cpus_allowed_list /proc/self/status >> ($seen)"] }
    }
    let cpus = { all: (allowed-cpus), one: (fastest-allowed-cpu) }
    let allowed = open /proc/self/status | lines | where $it starts-with Cpus_allowed_list | first

    time-group { name: all dir: $dir files: 0 } (pair $telling) 1 $cpus

    assert equal (open $seen | lines | uniq | sort) (
        [$allowed, $"Cpus_allowed_list:\t($cpus.one)"] | uniq | sort
    )
}

@test
def "time-group gives metrics, speedups, and gains of both formatters" [] {
    let context = $in
    let dir = $context.dir | path join staged
    put $dir a.nu "ls"
    let cpus = { all: (allowed-cpus), one: (fastest-allowed-cpu) }

    let actual = time-group { name: nu-std dir: $dir files: 1 } (pair $context.nuten) 1 $cpus

    let all_cpus = $actual.all_cpus.time_wall_clock
    let one_cpu = $actual.one_cpu.time_wall_clock
    assert equal $actual.name nu-std
    assert equal $actual.files 1
    assert equal ($actual.all_cpus | columns) [time_wall_clock time_cpu memory_peak_resident]
    assert ($actual.all_cpus.memory_peak_resident.nuten > 0b)
    assert equal $all_cpus.speedup ($all_cpus.nufmt / $all_cpus.nuten)
    assert equal $one_cpu.speedup ($one_cpu.nufmt / $one_cpu.nuten)
    assert equal $actual.one_cpu.cpu $cpus.one
    assert equal $actual.parallel_gain {
        nuten: ($one_cpu.nuten / $all_cpus.nuten)
        nufmt: ($one_cpu.nufmt / $all_cpus.nufmt)
    }
}

@test
def "time-group times nothing on one cpu without cpus to pin to" [] {
    let context = $in
    let dir = $context.dir | path join staged
    put $dir a.nu "ls"

    let actual = time-group { name: all dir: $dir files: 1 } (pair $context.nuten) 1 null

    assert ($actual.all_cpus.time_wall_clock.nuten > 0sec)
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
def "time-directories times groups in staged directory" [] {
    let context = $in
    let corpus = $context.dir | path join corpus
    let file = put $corpus nu-std/a.nu "ls"

    let actual = time-directories [$file] $corpus (pair $context.nuten) 1 null

    assert equal ($actual | get name) [nu-std all]
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
def "compare takes files under corpus and those also given" [] {
    let context = $in
    let corpus = $context.dir | path join corpus
    let inside = put $corpus sub/a.nu "ls"
    let after = put $context.dir b.nu "ls"

    let actual = compare $corpus $context.nuten.command $context.failing_nufmt $context.oracle [$after] 100sec 1

    assert equal $actual.compared []
    assert equal $actual.excluded [{ formatter: nufmt reason: fails files: [$after $inside] }]
}

@test
def "compare takes file given twice once" [] {
    let context = $in
    let file = put $context.dir a.nu "ls"

    let actual = compare $context.dir $context.nuten.command $context.failing_nufmt $context.oracle [$file] 100sec 1

    assert equal $actual.excluded [{ formatter: nufmt reason: fails files: [$file] }]
}

@test
def "compare takes relative paths from where it runs" [] {
    let context = $in
    let inside = put $context.dir corpus/nu-std/a.nu "ls"
    let after = put $context.dir b.nu "ls"
    ^ln -s $context.nuten.command ($context.dir | path join nuten)
    ^ln -s $context.oracle ($context.dir | path join oracle)
    let nufmt = keeping-nufmt $context.dir | path basename

    let actual = do {
        cd $context.dir
        compare corpus ./nuten $"./($nufmt)" ./oracle [b.nu] 100sec 1
    }

    assert equal $actual.compared [$after $inside]
    assert equal ($actual.directories | get name) [nu-std all]
}

@test
def "compare times files both keep on each file and directory" [] {
    let context = $in
    let file = put $context.dir nu-std/a.nu "ls"
    let nufmt = keeping-nufmt $context.dir

    let actual = compare $context.dir $context.nuten.command $nufmt $context.oracle [] 100sec 1

    assert equal ($actual.timing.files | get file) [$file]
    assert equal ($actual.directories | get name) [nu-std all]
}

@test
def "compare times no file and no directory when every file is left out" [] {
    let context = $in
    put $context.dir a.nu "ls"

    let actual = compare $context.dir $context.nuten.command $context.failing_nufmt $context.oracle [] 100sec 1

    assert equal $actual.timing { files: [] total: null speedup: null }
    assert equal $actual.directories []
}

@test
def "compare gives versions, machine, and binary sizes" [] {
    let context = $in
    put $context.dir a.nu "ls"
    let nodes = open --raw $LOCK | from json | get nodes
    let size = ls $context.nuten.command | get 0.size

    let actual = compare $context.dir $context.nuten.command $context.failing_nufmt $context.oracle [] 100sec 1

    assert equal $actual.versions.nufmt.rev $nodes.nufmt.locked.rev
    assert equal $actual.environment.cpus (sys cpu | length)
    assert equal $actual.sizes { nuten: $size nufmt: $size }
}

@test
def "main nufmt exits with 2 when asked for no run" [] {
    let context = $in

    let actual = ^$nu.current-exe $MAIN nufmt $context.dir a b c --runs 0
        | complete

    assert equal $actual.exit_code 2
    assert str contains $actual.stderr "--runs must be at least 1"
}

@test
def "main nufmt exits with 2 naming tools not found" [] {
    let context = $in

    let actual = with-env { PATH: [] } {
        ^$nu.current-exe $MAIN nufmt $context.dir a b c | complete
    }

    assert equal $actual.exit_code 2
    assert str contains $actual.stderr "timeout, hyperfine not found"
}
