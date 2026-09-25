# Compares nuten with nufmt, over only the files both format without changing what nu parses.
# A formatter must not win time on a file it fails on or reads differently, so such files are left out for both.

# Prints which files the comparison takes, and which each formatter leaves out and why.
def main [
    corpus: path # The corpus directory, as NUTEN_CORPUS names it
    ...also: path # More files to compare, such as the one nufmt benchmarks itself on
    --nuten: path # The nuten binary
    --nufmt: path # The nufmt binary
    --oracle: path # The nuten-oracle binary, which asks nu whether formatting keeps the AST
    --time-limit: duration = 100sec # How long a formatter may take on one file before it is cut off
]: nothing -> record {
    let missing = [[flag value]; [--nuten $nuten] [--nufmt $nufmt] [--oracle $oracle]]
    | where value == null
    | get flag
    if not ($missing | is-empty) {
        print --stderr $"($missing | str join ', ') not given"
        exit 2
    }
    if (which timeout | is-empty) {
        print --stderr "timeout not found"
        exit 2
    }

    # Each formatter runs in the directory of its file, where a relative path would lead elsewhere.
    let formatters = formatters ($nuten | path expand) ($nufmt | path expand)
    let oracle = $oracle | path expand

    [...(glob ($corpus | path join '**/*.nu')) ...($also | path expand)]
    | par-each {|file| verdict $file $formatters $oracle $time_limit }
    | sort-by file
    | report
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
