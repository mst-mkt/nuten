use comparable.nu screen
use hyperfine.nu medians
use provenance.nu [environment, versions]
use staging.nu stage
use valgrind.nu count-instructions

const REPOSITORY = path self ../../..
const LOCK = path self ../../../nix/dev/flake.lock
const FILE_METRICS = [time_wall_clock, memory_peak_resident]
const DIRECTORY_METRICS = [time_wall_clock, time_cpu, memory_peak_resident]
const SPEEDUP_METRICS = [time_wall_clock, instructions]
const SCHEMA = 1
const VALGRIND_SLOWDOWN = 100

export def compare [
    corpus: path
    nuten: path
    nufmt: path
    oracle: path
    also: list<path>
    time_limit: duration
    runs: int
    cpu: oneof<int, nothing>
]: nothing -> record {
    let measured_at = date now
    let formatters = formatters ($nuten | path expand) ($nufmt | path expand)
    let corpus = $corpus | path expand
    let files = [...(glob ($corpus | path join '**/*.nu')), ...($also | path expand)] | uniq
    let selection = screen $files $formatters ($oracle | path expand) $time_limit
    let instructions = instructions-of-files $selection.compared $formatters (
        $time_limit * $VALGRIND_SLOWDOWN
    )
    # One file or directory at a time, since formatters timed side by side would slow each other down.
    let timing = $selection.compared
        | wrap file
        | merge ($instructions | wrap instructions)
        | each {|row|
              time-file $row.file $formatters $runs --cpu $cpu
              | insert size (ls ($row.file | path expand) | get 0.size)
              | insert instructions $row.instructions
          }
        | summarize
    let directories = time-directories $selection.compared $corpus $formatters $runs $cpu

    {
        schema: $SCHEMA
        measured_at: $measured_at
        runs: $runs
        versions: (versions $REPOSITORY $LOCK)
        environment: (environment)
        files: ($files | length)
        excluded: ($selection.excluded | each { update files { each { relative $corpus } } })
        unreadable: ($selection.unreadable | each { relative $corpus })
        timing: ($timing | update files { each { update file { relative $corpus } } })
        directories: $directories
        sizes: (sizes $formatters)
    }
}

def relative [corpus: path]: path -> path {
    let file = $in

    if ($file | str starts-with $"($corpus)/") {
        $file | path relative-to $corpus
    } else {
        $file
    }
}

def formatters [nuten: path, nufmt: path]: nothing -> list<record> {
    [
        {
            name: nuten
            command: $nuten
            file: {|file| [$nuten, format, --stdin-filepath, $file, -] }
            directory: {|dir| [$nuten, format, --list-different, $dir] }
            rejection_exit_code: 2
        }
        {
            name: nufmt
            command: $nufmt
            file: {|file| [$nufmt, --stdin] }
            directory: {|dir| [$nufmt, --dry-run, $dir] }
        }
    ]
}

def time-file [file: path, formatters: list<record>, runs: int, --cpu: int]: nothing -> record {
    cd ($file | path dirname)
    let commands = $formatters
        | each {|formatter| { name: $formatter.name, words: (do $formatter.file $file) } }
    let measured = medians $commands $runs [--input, $file] $file --cpu $cpu

    { file: $file, ...($measured | select ...$FILE_METRICS | with-speedups) }
}

# Valgrind counts the same however many runs share the CPUs, so files are counted side by side.
def instructions-of-files [
    files: list<path>
    formatters: list<record>
    limit: duration
]: nothing -> list<record> {
    $files | par-each --keep-order {|file| instructions-of-file $file $formatters $limit }
}

def instructions-of-file [
    file: path
    formatters: list<record>
    limit: duration
]: nothing -> record {
    cd ($file | path dirname)
    let source = open --raw $file
    let counts = $formatters
        | each {|formatter|
              let run = {
                  name: $"($formatter.name) with ($file)"
                  words: (do $formatter.file $file)
                  stdin: $source
                  exit_codes: [0]
              }
              { name: $formatter.name, count: (count-instructions $run $limit) }
          }
    let timed_out = $counts | where count == null | get name
    if not ($timed_out | is-empty) {
        error make { msg: $"($timed_out | str join ', ') timed out under valgrind on ($file)" }
    }

    $counts | each { [$in.name, $in.count] } | into record | with-speedup
}

# Only the compared files are staged, so neither formatter meets a file left out of the comparison.
def time-directories [
    files: list<path>
    corpus: path
    formatters: list<record>
    runs: int
    cpu: oneof<int, nothing>
]: nothing -> list<record> {
    let staged = stage $files $corpus
    try {
        let directories = groups $staged | each {|group| time-group $group $formatters $runs $cpu }
        rm --recursive $staged

        $directories
    } catch {|error|
        rm --recursive $staged
        error make $error.raw
    }
}

def groups [staged: path]: nothing -> table {
    [[name, dir]; [nu-std, ($staged | path join nu-std)], [all, $staged]]
    | insert files {|group| glob ($group.dir | path join '**/*.nu') | length }
    | where files > 0
}

def time-group [
    group: record
    formatters: list<record>
    runs: int
    cpu: oneof<int, nothing>
]: nothing -> record {
    let all_cpus = time-directory $group.dir $formatters $runs
        | select ...$DIRECTORY_METRICS
        | with-speedups
    let one_cpu = if $cpu == null {
        null
    } else {
        time-directory $group.dir $formatters $runs --cpu $cpu
        | select ...$DIRECTORY_METRICS
        | with-speedups
        | insert cpu $cpu
    }

    {
        name: $group.name
        files: $group.files
        all_cpus: $all_cpus
        one_cpu: $one_cpu
        parallel_gain: (parallel-gain $all_cpus.time_wall_clock $one_cpu.time_wall_clock?)
    }
}

def parallel-gain [
    all_cpus: record
    one_cpu: oneof<record, nothing>
]: nothing -> oneof<record, nothing> {
    if $one_cpu == null {
        return null
    }

    { nuten: ($one_cpu.nuten / $all_cpus.nuten), nufmt: ($one_cpu.nufmt / $all_cpus.nufmt) }
}

def time-directory [
    dir: path
    formatters: list<record>
    runs: int
    --cpu: int
]: nothing -> record {
    cd $dir
    let commands = $formatters
        | each {|formatter| { name: $formatter.name, words: (do $formatter.directory $dir) } }

    medians $commands $runs [--ignore-failure=1] $dir --cpu $cpu
}

def sizes [formatters: list<record>]: nothing -> record {
    $formatters
    | each {|formatter| { name: $formatter.name, size: (ls $formatter.command | get 0.size) } }
    | transpose --header-row --as-record
}

def with-speedup []: record -> record {
    insert speedup ($in.nufmt / $in.nuten)
}

def with-speedups []: record -> record {
    let measured = $in

    $SPEEDUP_METRICS
    | where $it in ($measured | columns)
    | reduce --fold $measured {|metric, sped| $sped | update $metric { with-speedup } }
}

def summarize []: list<record> -> record {
    let files = $in
    if ($files | is-empty) {
        return { files: [], total: null, speedup: null }
    }
    let metrics = $SPEEDUP_METRICS | where {|metric| $files | all { $metric in ($in | columns) } }

    {
        files: $files
        total: (
            $metrics
            | each {|metric| [$metric, ($files | get $metric | select nuten nufmt | math sum)] }
            | into record
        )
        speedup: (
            $metrics
            | each {|metric| [$metric, ($files | get $metric | get speedup | median-and-p10)] }
            | into record
        )
    }
}

def median-and-p10 []: list<number> -> record {
    { median: ($in | math median), p10: ($in | percentile 10) }
}

def percentile [percent: number]: list<number> -> number {
    let sorted = $in | sort
    let rank = ($percent * ($sorted | length) / 100 | math ceil) - 1

    $sorted | get ([$rank, 0] | math max)
}
