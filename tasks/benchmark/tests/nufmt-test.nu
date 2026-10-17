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

def measured [instructions: record]: nothing -> record {
    {
        time_wall_clock: { nuten: 1ms, nufmt: 2ms }
        time_cpu: { nuten: 1ms, nufmt: 2ms }
        memory_peak_resident: { nuten: 1MB, nufmt: 2MB }
        time_user: { nuten: 1ms, nufmt: 2ms }
        instructions: $instructions
    }
}

def pair [formatter: record]: nothing -> list<record> {
    [$formatter, ($formatter | update name nufmt)]
}

def times []: nothing -> list<record> {
    [
        {
            file: a.nu
            time_wall_clock: { nuten: 10ms, nufmt: 40ms, speedup: 4.0 }
            instructions: { nuten: 10, nufmt: 20, speedup: 2.0 }
        }
        {
            file: b.nu
            time_wall_clock: { nuten: 20ms, nufmt: 20ms, speedup: 1.0 }
            instructions: { nuten: 10, nufmt: 10, speedup: 1.0 }
        }
        {
            file: c.nu
            time_wall_clock: { nuten: 20ms, nufmt: 30ms, speedup: 1.5 }
            instructions: { nuten: 10, nufmt: 30, speedup: 3.0 }
        }
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
def "formatters gives rejection code of nuten and none of nufmt" [] {
    let actual = formatters /bin/nuten /bin/nufmt

    assert equal $actual.0.rejection_exit_code 2
    assert equal $actual.1.rejection_exit_code? null
}

@test
def "time-file times each formatter on file" [] {
    let context = $in
    let file = put $context.dir a.nu "ls|length"

    let actual = time-file $file (pair $context.nuten) 1

    assert equal ($actual | columns) [file time_wall_clock memory_peak_resident]
    assert equal $actual.file $file
    assert ($actual.time_wall_clock.nufmt > 0sec)
    assert equal $actual.time_wall_clock.speedup (
        $actual.time_wall_clock.nufmt / $actual.time_wall_clock.nuten
    )
    assert ($actual.memory_peak_resident.nuten > 0b)
}

@test
def "time-file runs formatter in directory of file with file on stdin" [] {
    let context = $in
    let file = put $context.dir sub/a.nu "ls"
    let telling = { name: nuten, file: {|file| [sh, -c, "pwd > ran-in; cat > got"] } }

    time-file $file (pair $telling) 1

    assert (($context.dir | path join sub ran-in) | path exists)
    assert equal (open --raw ($context.dir | path join sub got)) "ls"
}

@test
def "time-file runs formatter on cpu given" [] {
    let context = $in
    let file = put $context.dir a.nu "ls"
    let telling = {
        name: nuten
        file: {|file| [sh, -c, "grep Cpus_allowed_list /proc/self/status > ran; cat"] }
    }
    let cpu = allowed-cpus | last

    time-file $file (pair $telling) 1 --cpu $cpu

    assert equal (open ($context.dir | path join ran) | str trim) $"Cpus_allowed_list:\t($cpu)"
}

@test
def "time-file times file whose path holds single quote" [] {
    let context = $in
    let file = put $context.dir "it's.nu" "ls"

    let actual = time-file $file (pair $context.nuten) 1

    assert ($actual.time_wall_clock.nuten > 0sec)
}

@test
def "instructions-of-file counts instructions of each formatter on file with speedup" [] {
    let context = $in
    let file = put $context.dir a.nu "ls|length"

    let actual = instructions-of-file $file (pair $context.nuten) 100sec

    assert equal ($actual | columns) [nuten nufmt speedup]
    assert ($actual.nuten > 0)
    assert ($actual.nufmt > 0)
    assert equal $actual.speedup ($actual.nufmt / $actual.nuten)
}

@test
def "instructions-of-file runs formatter in directory of file with file on stdin" [] {
    let context = $in
    let file = put $context.dir sub/a.nu "ls"
    let telling = { name: nuten, file: {|file| [sh, -c, "pwd > ran-in; cat > got"] } }

    instructions-of-file $file (pair $telling) 100sec

    assert equal (open ($context.dir | path join sub ran-in) | str trim) (
        $context.dir | path join sub
    )
    assert equal (open --raw ($context.dir | path join sub got)) "ls"
}

@test
def "instructions-of-file fails naming formatter that runs past time limit" [] {
    let context = $in
    let file = put $context.dir a.nu "ls"
    let formatters = [
        { name: nuten, file: {|file| [sleep, 30] } }
        { name: nufmt, file: {|file| [cat] } }
    ]

    let actual = try { instructions-of-file $file $formatters 20sec } catch {|error| $error.msg }

    assert equal $actual $"nuten timed out under valgrind on ($file)"
}

@test
def "instructions-of-file names every formatter that runs past time limit" [] {
    let context = $in
    let file = put $context.dir a.nu "ls"
    let sleeping = { name: nuten, file: {|file| [sleep, 30] } }

    let actual = try {
        instructions-of-file $file (pair $sleeping) 1sec
    } catch {|error|
        $error.msg
    }

    assert equal $actual $"nuten, nufmt timed out under valgrind on ($file)"
}

@test
def "instructions-of-file fails naming formatter that fails under valgrind" [] {
    let context = $in
    let file = put $context.dir a.nu "ls"
    let failing = { name: nuten, file: {|file| [sh, -c, "exit 2"] } }

    let actual = try {
        instructions-of-file $file (pair $failing) 100sec
    } catch {|error|
        $error.rendered | ansi strip
    }

    assert str contains $actual $"nuten with ($file) failed under valgrind"
}

@test
def "instructions-of-files keeps order of files that finish out of order" [] {
    let context = $in
    # The first file waits longest and counts most, so it finishes last.
    let files = [3, 2, 1] | each {|rank| put $context.dir $"($rank).nu" "" }
    let busy = {
        name: nuten
        file: {|file|
            let rank = $file | path parse | get stem | into int
            [
                sh
                -c
                $"sleep ($rank - 1); i=0; while [ $i -lt ($rank * 2000) ]; do i=$\(\(i + 1\)\); done"
            ]
        }
    }

    let actual = instructions-of-files $files (pair $busy) 100sec

    assert equal ($actual | get nuten) ($actual | get nuten | sort --reverse)
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

    time-directory $dir [$telling] 1

    assert equal (open ($dir | path join ran-in) | str trim) $dir
    assert equal (open ($dir | path join ran-on) | str trim) $dir
}

@test
def "time-directory takes check that finds change as success" [] {
    let context = $in
    let dir = $context.dir | path join staged
    put $dir a.nu "ls|length"

    let actual = time-directory $dir [$context.nuten] 1

    assert ($actual.time_wall_clock.nuten > 0sec)
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
    let cpu = fastest-allowed-cpu
    let allowed = open /proc/self/status | lines | where $it starts-with Cpus_allowed_list | first

    time-group { name: all dir: $dir files: 0 } (pair $telling) 1 $cpu

    assert equal (open $seen | lines | uniq | sort) (
        [$allowed, $"Cpus_allowed_list:\t($cpu)"] | uniq | sort
    )
}

@test
def "time-group gives metrics, speedups, and gains of both formatters" [] {
    let context = $in
    let dir = $context.dir | path join staged
    put $dir a.nu "ls"
    let cpu = fastest-allowed-cpu

    let actual = time-group { name: nu-std dir: $dir files: 1 } (pair $context.nuten) 1 $cpu

    let all_cpus = $actual.all_cpus.time_wall_clock
    let one_cpu = $actual.one_cpu.time_wall_clock
    assert equal $actual.name nu-std
    assert equal $actual.files 1
    assert equal ($actual.all_cpus | columns) [time_wall_clock time_cpu memory_peak_resident]
    assert equal ($actual.one_cpu | columns) [time_wall_clock time_cpu memory_peak_resident cpu]
    assert ($actual.all_cpus.memory_peak_resident.nuten > 0b)
    assert equal $all_cpus.speedup ($all_cpus.nufmt / $all_cpus.nuten)
    assert equal $one_cpu.speedup ($one_cpu.nufmt / $one_cpu.nuten)
    assert equal $actual.one_cpu.cpu $cpu
    assert equal $actual.parallel_gain {
        nuten: ($one_cpu.nuten / $all_cpus.nuten)
        nufmt: ($one_cpu.nufmt / $all_cpus.nufmt)
    }
}

@test
def "time-group times nothing on one cpu without cpu to pin to" [] {
    let context = $in
    let dir = $context.dir | path join staged
    put $dir a.nu "ls"

    let actual = time-group { name: all dir: $dir files: 1 } (pair $context.nuten) 1 null

    assert ($actual.all_cpus.time_wall_clock.nuten > 0sec)
    assert equal $actual.one_cpu null
    assert equal $actual.parallel_gain null
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
def "with-speedups gives speedup by wall time and instructions and no other metric" [] {
    let measured = measured { nuten: 2, nufmt: 3 }

    let actual = $measured | with-speedups

    assert equal $actual.time_wall_clock.speedup 2.0
    assert equal $actual.instructions.speedup 1.5
    assert ("speedup" not-in ($actual.memory_peak_resident | columns))
}

@test
def "summarize keeps each file" [] {
    let actual = times | summarize

    assert equal $actual.files (times)
}

@test
def "summarize totals wall time and instructions of each formatter" [] {
    let actual = times | summarize

    assert equal $actual.total {
        time_wall_clock: { nuten: 50ms nufmt: 90ms }
        instructions: { nuten: 30 nufmt: 60 }
    }
}

@test
def "summarize gives median speedup by wall time and instructions" [] {
    let actual = times | summarize

    assert equal $actual.speedup.time_wall_clock.median 1.5
    assert equal $actual.speedup.instructions.median 2.0
}

@test
def "summarize gives 10th percentile speedup" [] {
    let files = 1..20
        | each {|i| { file: $"($i).nu" time_wall_clock: { nuten: 10ms, nufmt: ($i * 10ms), speedup: $i } } }

    let actual = $files | summarize

    assert equal $actual.speedup.time_wall_clock.p10 2
}

@test
def "summarize gives speedup only by metrics every file has" [] {
    let files = times | update 1 { reject instructions }

    let actual = $files | summarize

    assert equal ($actual.speedup | columns) [time_wall_clock]
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

    let actual = compare $corpus $context.nuten.command $context.failing_nufmt $context.oracle [$after] 100sec 1 null

    assert equal $actual.timing.files []
    assert equal $actual.excluded [{ formatter: nufmt reason: fails files: [$after sub/a.nu] }]
}

@test
def "compare takes file given twice once" [] {
    let context = $in
    let file = put $context.dir a.nu "ls"

    let actual = compare $context.dir $context.nuten.command $context.failing_nufmt $context.oracle [$file] 100sec 1 null

    assert equal $actual.files 1
    assert equal $actual.excluded [{ formatter: nufmt reason: fails files: [a.nu] }]
}

@test
def "compare takes relative paths from where it runs" [] {
    let context = $in
    put $context.dir corpus/nu-std/a.nu "ls"
    let after = put $context.dir b.nu "ls"
    ^ln -s $context.nuten.command ($context.dir | path join nuten)
    ^ln -s $context.oracle ($context.dir | path join oracle)
    let nufmt = keeping-nufmt $context.dir | path basename

    let actual = do {
        cd $context.dir
        compare corpus ./nuten $"./($nufmt)" ./oracle [b.nu] 100sec 1 null
    }

    assert equal ($actual.timing.files | get file) [$after nu-std/a.nu]
    assert equal ($actual.directories | get name) [nu-std all]
}

@test
def "compare times files both keep on each file and directory, and directories on cpu given" [] {
    let context = $in
    put $context.dir nu-std/a.nu "ls"
    let nufmt = keeping-nufmt $context.dir
    let cpu = fastest-allowed-cpu

    let actual = compare $context.dir $context.nuten.command $nufmt $context.oracle [] 100sec 1 $cpu

    assert equal ($actual.timing.files | get file) [nu-std/a.nu]
    assert ($actual.timing.files.0.instructions.nuten > 0)
    assert equal ($actual.directories | get name) [nu-std all]
    assert equal $actual.directories.0.one_cpu.cpu $cpu
}

@test
def "compare times each file on cpu given" [] {
    let context = $in
    put $context.dir a.nu "ls"
    let script = "case \"$1\" in --stdin) grep Cpus_allowed_list /proc/self/status >> ran; cat ;; esac"
    let nufmt = put $context.dir nufmt $"#!/bin/sh\n($script)\n"
    chmod +x $nufmt
    let cpu = allowed-cpus | last

    compare $context.dir $context.nuten.command $nufmt $context.oracle [] 100sec 1 $cpu

    assert ($"Cpus_allowed_list:\t($cpu)" in (open ($context.dir | path join ran) | lines))
}

@test
def "compare times no file and no directory when every file is left out" [] {
    let context = $in
    put $context.dir a.nu "ls"

    let actual = compare $context.dir $context.nuten.command $context.failing_nufmt $context.oracle [] 100sec 1 null

    assert equal $actual.timing { files: [] total: null speedup: null }
    assert equal $actual.directories []
}

@test
def "compare gives versions, machine, and binary sizes" [] {
    let context = $in
    put $context.dir a.nu "ls"
    let nodes = open --raw $LOCK | from json | get nodes
    let size = ls $context.nuten.command | get 0.size

    let actual = compare $context.dir $context.nuten.command $context.failing_nufmt $context.oracle [] 100sec 1 null

    assert equal $actual.versions.nufmt.rev $nodes.nufmt.locked.rev
    assert equal $actual.versions.nushell.rev $nodes.nushell.locked.rev
    assert equal $actual.environment.cpus (sys cpu | length)
    assert equal $actual.sizes { nuten: $size nufmt: $size }
}

@test
def "compare gives schema, time it started, and runs" [] {
    let context = $in
    put $context.dir a.nu "ls"
    let telling = put $context.dir nufmt "#!/bin/sh\ndate +%s%N >> ran-at\nexit 1\n"
    chmod +x $telling
    let before = date now

    let actual = compare $context.dir $context.nuten.command $telling $context.oracle [] 100sec 3 null

    let ran_at = open ($context.dir | path join ran-at) | lines | first | into int | into datetime
    assert equal $actual.schema 1
    assert ($actual.measured_at >= $before and $actual.measured_at < $ran_at)
    assert equal $actual.runs 3
}

@test
def "compare gives instructions of each file on its row" [] {
    let context = $in
    put $context.dir a.nu (1..300 | each { "ls | length\n" } | str join)
    put $context.dir b.nu "ls"
    let nufmt = keeping-nufmt $context.dir

    let actual = compare $context.dir $context.nuten.command $nufmt $context.oracle [] 100sec 1 null

    let files = $actual.timing.files
    assert equal ($files | get file) [a.nu b.nu]
    assert ($files.0.instructions.nuten > $files.1.instructions.nuten)
}

@test
def "compare gives size of each file" [] {
    let context = $in
    put $context.dir a.nu "ls|length"
    let nufmt = keeping-nufmt $context.dir

    let actual = compare $context.dir $context.nuten.command $nufmt $context.oracle [] 100sec 1 null

    assert equal ($actual.timing.files | select file size) [{ file: a.nu size: 9b }]
}

@test
def "compare gives size of file symlink points to" [] {
    let context = $in
    put $context.dir a.txt "ls|length"
    ^ln -s a.txt ($context.dir | path join a.nu)
    let nufmt = keeping-nufmt $context.dir

    let actual = compare $context.dir $context.nuten.command $nufmt $context.oracle [] 100sec 1 null

    assert equal ($actual.timing.files | select file size) [{ file: a.nu size: 9b }]
}

@test
def "compare gives valgrind more time than formatter alone" [] {
    let context = $in
    put $context.dir a.nu "ls"
    # Valgrind preloads its own library into what it runs.
    let script = "case \"$LD_PRELOAD\" in *vgpreload*) sleep 2 ;; esac\ncat\n"
    let nufmt = put $context.dir nufmt $"#!/bin/sh\n($script)"
    chmod +x $nufmt

    let actual = compare $context.dir $context.nuten.command $nufmt $context.oracle [] 1sec 1 null

    assert equal ($actual.timing.files | get file) [a.nu]
}

@test
def "compare gives unreadable file relative to corpus" [] {
    let context = $in
    put $context.dir sub/a.nu (0x[ff fe])

    let actual = compare $context.dir $context.nuten.command $context.failing_nufmt $context.oracle [] 100sec 1 null

    assert equal $actual.unreadable [sub/a.nu]
}

@test
def "relative gives path of file in corpus from it" [] {
    let actual = "/c/corpus/sub/a.nu" | relative /c/corpus

    assert equal $actual sub/a.nu
}

@test
def "relative keeps path of file outside corpus" [] {
    let actual = "/c/corpus2/a.nu" | relative /c/corpus

    assert equal $actual /c/corpus2/a.nu
}

@test
def "main nufmt times directories on fastest allowed cpu" [] {
    let context = $in
    put $context.dir a.nu "ls"
    let nufmt = keeping-nufmt $context.dir
    let paths = [$context.dir, $context.nuten.command, $nufmt, $context.oracle]
        | each { to nuon }
        | str join ' '
    let script = $"source ($MAIN | to nuon); main nufmt ($paths) --runs 1 | to nuon"

    let actual = ^$nu.current-exe --no-config-file --commands $script | complete

    assert equal $actual.exit_code 0 $actual.stderr
    assert equal ($actual.stdout | from nuon | get directories.one_cpu.cpu) [(fastest-allowed-cpu)]
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
    assert str contains $actual.stderr "timeout, hyperfine, valgrind, taskset not found"
}
