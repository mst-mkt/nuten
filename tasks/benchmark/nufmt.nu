# Compares nuten with nufmt, over only the files both format without changing what nu parses.
# A formatter must not win time on a file it fails on or reads differently, so such files are left out for both.

# Prints which files the comparison takes, which each formatter leaves out and why, and how long each formatter takes on the files compared.
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

    let selection = [...(glob ($corpus | path join '**/*.nu')) ...($also | path expand)]
    | par-each {|file| verdict $file $formatters $oracle $time_limit }
    | sort-by file
    | report
    # One file at a time, since formatters timed side by side would slow each other down.
    let timing = $selection.compared | each {|file| time-file $file $formatters $runs } | summarize

    { ...$selection timing: $timing }
}

def formatters [nuten: path nufmt: path]: nothing -> list<record> {
    [
        { name: nuten command: $nuten arguments: {|file| [format --stdin-filepath $file -] } }
        { name: nufmt command: $nufmt arguments: {|file| [--stdin] } }
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

# The median shrugs off a run slowed by something else on the machine.
def time-file [file: path formatters: list<record> runs: int]: nothing -> record {
    cd ($file | path dirname)
    let export = mktemp --tmpdir --suffix .json
    let commands = $formatters
    | each {|formatter| [--command-name $formatter.name (command-line $formatter $file)] }
    | flatten
    let result = ^hyperfine -N --input $file --warmup 1 --runs $runs --export-json $export ...$commands
    | complete
    let exported = open --raw $export
    rm $export
    if $result.exit_code != 0 {
        error make {
            msg: $"hyperfine failed on ($file) with exit code ($result.exit_code): ($result.stderr)"
        }
    }
    let medians = $exported
    | from json
    | get results
    | select command median
    | update median { $in * 1sec }
    | transpose --header-row --as-record

    { file: $file ...$medians }
}

# Hyperfine splits a command without a shell into words as a shell would, so each is quoted.
def command-line [formatter: record file: path]: nothing -> string {
    [$formatter.command ...(do $formatter.arguments $file)]
    | each { $"'($in | str replace --all "'" "'\\''")'" }
    | str join ' '
}

def summarize []: list<record> -> record {
    let files = $in | each {|timed| $timed | insert speedup ($timed.nufmt / $timed.nuten) }
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
