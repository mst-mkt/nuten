# A formatter must not win time on a file it fails on or whose AST it changes, so such a file is left out for both.
export def screen [
    files: list<path>
    formatters: list<record>
    oracle: path
    limit: duration
]: nothing -> record {
    $files
    | par-each {|file| verdict $file $formatters $oracle $limit }
    | sort-by file
    | report
}

def verdict [
    file: path
    formatters: list<record>
    oracle: path
    limit: duration
]: nothing -> record {
    let source = open --raw $file
    if ($source | describe) != string {
        return { file: $file, unreadable: true, problems: [] }
    }
    let problems = $formatters
        | each {|formatter|
              let reason = problem $file $source $formatter $oracle $limit
              if $reason != null {
                  { formatter: $formatter.name, reason: $reason }
              }
          }
        | compact

    { file: $file, unreadable: false, problems: $problems }
}

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
        | ^timeout $seconds ...(do $formatter.file $file)
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
          { formatter: $group.formatter, reason: $group.reason, files: ($group.items | get file) }
      }
}
