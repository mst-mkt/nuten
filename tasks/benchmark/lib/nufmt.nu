use comparable.nu screen
use hyperfine.nu medians
use provenance.nu [environment, versions]
use staging.nu stage

const REPOSITORY = path self ../../..
const LOCK = path self ../../../nix/dev/flake.lock
const DIRECTORY_METRICS = [time_wall_clock, time_cpu, memory_peak_resident]

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
    let formatters = formatters ($nuten | path expand) ($nufmt | path expand)
    let corpus = $corpus | path expand
    let files = [...(glob ($corpus | path join '**/*.nu')), ...($also | path expand)] | uniq
    let selection = screen $files $formatters ($oracle | path expand) $time_limit
    # One file or directory at a time, since formatters timed side by side would slow each other down.
    let timing = $selection.compared
        | each {|file| time-file $file $formatters $runs --cpu $cpu }
        | summarize
    let directories = time-directories $selection.compared $corpus $formatters $runs $cpu

    {
        versions: (versions $REPOSITORY $LOCK)
        environment: (environment)
        ...$selection
        timing: $timing
        directories: $directories
        sizes: (sizes $formatters)
    }
}

def formatters [nuten: path, nufmt: path]: nothing -> list<record> {
    [
        {
            name: nuten
            command: $nuten
            file: {|file| [$nuten, format, --stdin-filepath, $file, -] }
            directory: {|dir| [$nuten, format, --list-different, $dir] }
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
    let timed = medians $commands $runs [--input, $file] $file --cpu $cpu

    { file: $file, ...$timed.time_wall_clock }
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
        | update time_wall_clock { with-speedup }
    let one_cpu = if $cpu == null {
        null
    } else {
        time-directory $group.dir $formatters $runs --cpu $cpu
        | one-cpu-metrics
        | update time_wall_clock { with-speedup }
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

# Hyperfine counts instructions on only one kind of core of a hybrid CPU, and reads zero on the other.
def one-cpu-metrics []: record -> record {
    let measured = $in
    let counted = $measured.instructions?
        | default { nuten: 0, nufmt: 0 }
        | values
        | all { $in > 0 }
    let metrics = if $counted { [...$DIRECTORY_METRICS, instructions] } else { $DIRECTORY_METRICS }

    $measured | select ...$metrics
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

def summarize []: list<record> -> record {
    let files = $in | each { with-speedup }
    if ($files | is-empty) {
        return { files: [], total: null, speedup: null }
    }

    {
        files: $files
        total: { nuten: ($files.nuten | math sum), nufmt: ($files.nufmt | math sum) }
        speedup: {
            median: ($files.speedup | math median)
            p10: ($files.speedup | percentile 10)
        }
    }
}

def percentile [percent: number]: list<number> -> number {
    let sorted = $in | sort
    let rank = ($percent * ($sorted | length) / 100 | math ceil) - 1

    $sorted | get ([$rank, 0] | math max)
}
