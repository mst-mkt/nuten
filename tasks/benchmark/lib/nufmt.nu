use comparable.nu screen
use cpus.nu [allowed-cpus, fastest-allowed-cpu]
use provenance.nu [environment, versions]
use staging.nu stage

const REPOSITORY = path self ../../..
const LOCK = path self ../../../nix/dev/flake.lock

export def compare [
    corpus: path
    nuten: path
    nufmt: path
    oracle: path
    also: list<path>
    time_limit: duration
    runs: int
]: nothing -> record {
    let formatters = formatters ($nuten | path expand) ($nufmt | path expand)
    let corpus = $corpus | path expand
    let files = [...(glob ($corpus | path join '**/*.nu')), ...($also | path expand)] | uniq
    let selection = screen $files $formatters ($oracle | path expand) $time_limit
    let cpus = if (which taskset | is-empty) {
        null
    } else {
        { all: (allowed-cpus), one: (fastest-allowed-cpu) }
    }
    # One file or directory at a time, since formatters timed side by side would slow each other down.
    let timing = $selection.compared | each {|file| time-file $file $formatters $runs } | summarize
    let directories = time-directories $selection.compared $corpus $formatters $runs $cpus

    {
        versions: (versions $REPOSITORY $LOCK)
        environment: (environment)
        ...$selection
        timing: $timing
        ...$directories
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

def time-file [file: path, formatters: list<record>, runs: int]: nothing -> record {
    cd ($file | path dirname)
    let commands = $formatters
        | each {|formatter| { name: $formatter.name, words: (do $formatter.file $file) } }

    { file: $file, ...(time-commands $commands $runs [--input $file] $file) }
}

# Only the compared files are staged, so neither formatter meets a file left out of the comparison.
def time-directories [
    files: list<path>
    corpus: path
    formatters: list<record>
    runs: int
    cpus: oneof<record, nothing>
]: nothing -> record {
    let staged = stage $files $corpus
    try {
        let groups = groups $staged
        let directories = $groups | each {|group| time-group $group $formatters $runs $cpus }
        let memory = if ($groups | is-empty) { null } else { memory $staged $formatters }
        rm --recursive $staged

        { directories: $directories, memory: $memory }
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
    cpus: oneof<record, nothing>
]: nothing -> record {
    let all_cpus = time-directory $group.dir $formatters $runs (pin $cpus.all?) | with-speedup
    let one_cpu = if $cpus == null {
        null
    } else {
        time-directory $group.dir $formatters $runs (pin [$cpus.one])
        | with-speedup
        | insert cpu $cpus.one
    }

    {
        name: $group.name
        files: $group.files
        all_cpus: $all_cpus
        one_cpu: $one_cpu
        parallel_gain: (parallel-gain $all_cpus $one_cpu)
    }
}

# Taskset runs before the formatter on all CPUs too, so that it weighs the same on both sides of the gain.
def pin [cpus: oneof<list<int>, nothing>]: nothing -> list<string> {
    if $cpus == null { [] } else { [taskset, --cpu-list, ($cpus | str join ',')] }
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
    prefix: list<string>
]: nothing -> record {
    cd $dir
    let commands = $formatters
        | each {|formatter| { name: $formatter.name, words: [...$prefix ...(do $formatter.directory $dir)] } }

    time-commands $commands $runs [--ignore-failure=1] $dir
}

def time-commands [
    commands: list<record>
    runs: int
    options: list<string>
    subject: path
]: nothing -> record {
    let export = mktemp --tmpdir --suffix .json
    let named = $commands
        | each {|command| [--command-name $command.name (command-line $command.words)] }
        | flatten
    let result = ^hyperfine -N --warmup 1 --runs $runs --export-json $export ...$options ...$named
        | complete
    let exported = open --raw $export
    rm $export
    if $result.exit_code != 0 {
        error make {
            msg: $"hyperfine failed on ($subject) with exit code ($result.exit_code): ($result.stderr)"
        }
    }

    $exported
    | from json
    | get results
    | each {|result| { name: $result.name median: ($result.summary.time_wall_clock.median * 1sec) } }
    | transpose --header-row --as-record
}

def command-line [words: list<string>]: nothing -> string {
    $words
    | each { $"'($in | str replace --all "'" "'\\''")'" }
    | str join ' '
}

# One run each, since peak memory, unlike time, barely moves from run to run.
# macOS time cannot write the peak to a file of its own, so nothing is weighed there.
def memory [dir: path, formatters: list<record>]: nothing -> oneof<record, nothing> {
    if not (has-gnu-time) {
        return null
    }
    cd $dir

    $formatters
    | each {|formatter| { name: $formatter.name peak: (peak $formatter $dir) } }
    # Transpose lets an error in a stream it reads slip past a caller's try, so the stream is collected first.
    | collect
    | transpose --header-row --as-record
}

def has-gnu-time []: nothing -> bool {
    if (which time | is-empty) {
        return false
    }
    let version = ^time --version | complete

    $version.exit_code == 0 and ($version.stdout | str contains 'GNU Time')
}

# The peak goes to a file of its own, so what the formatter writes to stderr does not mix in.
def peak [formatter: record, dir: path]: nothing -> filesize {
    let report = mktemp --tmpdir
    let run = ^time -f %M -o $report ...(do $formatter.directory $dir) | complete
    let peak = open --raw $report
    rm $report
    # Both exit with 1 when a file would change, which is no failure here.
    if $run.exit_code not-in [0, 1] {
        error make {
            msg: $"($formatter.name) failed on ($dir) with exit code ($run.exit_code): ($run.stderr)"
        }
    }

    $peak | max-rss
}

# GNU time puts a line about a failing exit before the peak, so the peak is read from the last line.
def max-rss []: string -> filesize {
    lines | last | into int | $in * 1KiB
}

def sizes [formatters: list<record>]: nothing -> record {
    $formatters
    | each {|formatter| { name: $formatter.name size: (ls $formatter.command | get 0.size) } }
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
