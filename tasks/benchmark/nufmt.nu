# Compares nuten with nufmt, over only the files both format without changing what nu parses.
# A formatter must not win time on a file it fails on or reads differently, so such files are left out for both.

use cpus.nu [allowed-cpus fastest-allowed-cpu]

# Prints which files the comparison takes, which each formatter leaves out and why, and how both fare on the files compared.
def main [
    corpus: path # The corpus directory, as NUTEN_CORPUS names it
    ...also: path # More files to compare, such as the one nufmt benchmarks itself on
    --nuten: path # The nuten binary
    --nufmt: path # The nufmt binary
    --oracle: path # The nuten-oracle binary, which asks nu whether formatting keeps the AST
    --time-limit: duration = 100sec # How long a formatter may take on one file before it is cut off
    --runs: int = 5 # How many times each formatter is timed on each file
]: nothing -> record {
    let missing_flags = [[flag value]; [--nuten $nuten] [--nufmt $nufmt] [--oracle $oracle]]
    | where value == null
    | get flag
    if not ($missing_flags | is-empty) {
        print --stderr $"($missing_flags | str join ', ') not given"
        exit 2
    }
    # Hyperfine hangs when asked for no run.
    if $runs < 1 {
        print --stderr "--runs must be at least 1"
        exit 2
    }
    let missing_tools = [timeout hyperfine] | where { which $in | is-empty }
    if not ($missing_tools | is-empty) {
        print --stderr $"($missing_tools | str join ', ') not found"
        exit 2
    }

    # Each formatter runs in the directory of its file, where a relative path would lead elsewhere.
    let formatters = formatters ($nuten | path expand) ($nufmt | path expand)
    let oracle = $oracle | path expand
    let corpus = $corpus | path expand
    let sizes = sizes $formatters
    # macOS has no taskset, so nothing is timed on one CPU there.
    let cpus = if (which taskset | is-empty) {
        null
    } else {
        { all: (allowed-cpus) one: (fastest-allowed-cpu) }
    }

    let selection = [...(glob ($corpus | path join '**/*.nu')) ...($also | path expand)]
    | uniq
    | par-each {|file| verdict $file $formatters $oracle $time_limit }
    | sort-by file
    | report
    # One file or directory at a time, since formatters timed side by side would slow each other down.
    let timing = $selection.compared | each {|file| time-file $file $formatters $runs } | summarize
    let directories = time-directories $selection.compared $corpus $formatters $runs $cpus

    {
        ...$selection
        timing: $timing
        ...$directories
        sizes: $sizes
    }
}

def formatters [nuten: path nufmt: path]: nothing -> list<record> {
    [
        {
            name: nuten
            command: $nuten
            arguments: {|file| [format --stdin-filepath $file -] }
            directory: {|dir| [format --check $dir] }
        }
        {
            name: nufmt
            command: $nufmt
            arguments: {|file| [--stdin] }
            directory: {|dir| [--dry-run $dir] }
        }
    ]
}

# Nu reads no file that is not UTF-8, so neither formatter is asked about one.
def verdict [file: path formatters: list<record> oracle: path limit: duration]: nothing -> record {
    let source = open --raw $file
    if ($source | describe) != string {
        return { file: $file unreadable: true problems: [] }
    }
    let problems = $formatters
    | each {|formatter|
          let reason = problem $file $source $formatter $oracle $limit
          if $reason != null {
              { formatter: $formatter.name reason: $reason }
          }
      }
    | compact

    { file: $file unreadable: false problems: $problems }
}

# Running in the directory of the file, nufmt finds a config from there as nuten does by the path.
def problem [
    file: path
    source: string
    formatter: record
    oracle: path
    limit: duration
]: nothing -> oneof<string, nothing> {
    cd ($file | path dirname)
    let seconds = $limit / 1sec
    let formatted = $source
    | ^timeout $seconds $formatter.command ...(do $formatter.arguments $file)
    | complete
    match $formatted.exit_code {
        0 => {}
        124 => { return "times out" }
        _ => { return fails }
    }
    let output = mktemp --tmpdir --suffix .nu
    $formatted.stdout | save --force $output
    let compared = ^$oracle compare $file $output | complete
    rm $output

    match $compared.exit_code {
        0 => null
        1 => "changes the AST"
        _ => { error make { msg: $"nuten-oracle failed on ($file): ($compared.stderr)" } }
    }
}

def report []: list<record> -> record {
    let verdicts = $in

    {
        compared: ($verdicts | where not $it.unreadable and ($it.problems | is-empty) | get file)
        excluded: ($verdicts | excluded)
        unreadable: ($verdicts | where unreadable | get file)
    }
}

def excluded []: list<record> -> table {
    each {|verdict| $verdict.problems | insert file $verdict.file }
    | flatten
    | group-by formatter reason --to-table
    | each {|group|
          { formatter: $group.formatter reason: $group.reason files: ($group.items | get file) }
      }
}

def time-file [file: path formatters: list<record> runs: int]: nothing -> record {
    cd ($file | path dirname)
    let commands = $formatters
    | each {|formatter| { name: $formatter.name words: [$formatter.command ...(do $formatter.arguments $file)] } }

    { file: $file ...(time-commands $commands $runs [--input $file] $file) }
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

        { directories: $directories memory: $memory }
    } catch {|error|
        rm --recursive $staged
        error make $error.raw
    }
}

def stage [files: list<path> corpus: path]: nothing -> path {
    let root = mktemp --directory
    $files
    | enumerate
    | each {|entry|
        let target = $root | path join (staged-path $entry.item $corpus $entry.index)
        mkdir ($target | path dirname)
        cp $entry.item $target
    }
    | ignore

    $root
}

# A leading dot is dropped from every part, since nufmt skips hidden files and directories where nuten does not.
# A file from outside the corpus gets a directory of its own, so two of the same name do not meet.
def staged-path [file: path corpus: path index: int]: nothing -> path {
    let relative = if ($file | str starts-with $"($corpus)/") {
        $file | path relative-to $corpus
    } else {
        [also ($index | into string) ($file | path basename)] | path join
    }

    $relative
    | path split
    | each { if ($in | str starts-with '.') { $"_($in | str substring 1..)" } else { $in } }
    | path join
}

def groups [staged: path]: nothing -> table {
    [[name dir]; [nu-std ($staged | path join nu-std)] [all $staged]]
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
    if $cpus == null { [] } else { [taskset --cpu-list ($cpus | str join ',')] }
}

def parallel-gain [
    all_cpus: record
    one_cpu: oneof<record, nothing>
]: nothing -> oneof<record, nothing> {
    if $one_cpu == null {
        return null
    }

    { nuten: ($one_cpu.nuten / $all_cpus.nuten) nufmt: ($one_cpu.nufmt / $all_cpus.nufmt) }
}

def time-directory [
    dir: path
    formatters: list<record>
    runs: int
    prefix: list<string>
]: nothing -> record {
    cd $dir
    let commands = $formatters
    | each {|formatter|
        { name: $formatter.name words: [...$prefix $formatter.command ...(do $formatter.directory $dir)] }
    }

    # Both exit with 1 when a file would change, which is no failure here.
    time-commands $commands $runs [--ignore-failure=1] $dir
}

# The median shrugs off a run slowed by something else on the machine.
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
    | select command median
    | update median { $in * 1sec }
    | transpose --header-row --as-record
}

# Hyperfine splits a command without a shell into words as a shell would, so each is quoted.
def command-line [words: list<string>]: nothing -> string {
    $words
    | each { $"'($in | str replace --all "'" "'\\''")'" }
    | str join ' '
}

# One run each, since peak memory, unlike time, barely moves from run to run.
# macOS time cannot write the peak to a file of its own, so nothing is weighed there.
def memory [dir: path formatters: list<record>]: nothing -> oneof<record, nothing> {
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
def peak [formatter: record dir: path]: nothing -> filesize {
    let report = mktemp --tmpdir
    let run = ^time -f %M -o $report $formatter.command ...(do $formatter.directory $dir) | complete
    let peak = open --raw $report
    rm $report
    # Both exit with 1 when a file would change, which is no failure here.
    if $run.exit_code not-in [0 1] {
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
    # math median fails on an empty list.
    if ($files | is-empty) {
        return { files: [] total: null speedup: null }
    }

    {
        files: $files
        total: { nuten: ($files.nuten | math sum) nufmt: ($files.nufmt | math sum) }
        speedup: {
            median: ($files.speedup | math median)
            p10: ($files.speedup | percentile 10)
        }
    }
}

# Nearest rank rather than interpolation, so the result is always one of the values.
# Multiplying before dividing keeps a whole rank whole, where float error would let math ceil push it up.
def percentile [percent: number]: list<number> -> number {
    let sorted = $in | sort
    let rank = ($percent * ($sorted | length) / 100 | math ceil) - 1

    $sorted | get ([$rank 0] | math max)
}
