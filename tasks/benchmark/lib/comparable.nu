# A formatter must not win time on a file it fails on or whose AST it changes, so such a file is left out for both.
export def screen [
    files: list<path>
    formatters: list<record>
    oracle: path
    limit: duration
    max_size: filesize
]: nothing -> record {
    $files
    | par-each {|file| verdict $file $formatters $oracle $limit $max_size }
    | sort-by file
    | report
}

# Nu takes gigabytes of memory to write the AST of a megabyte file, so the AST of a file over max_size goes unchecked.
def verdict [
    file: path
    formatters: list<record>
    oracle: path
    limit: duration
    max_size: filesize
]: nothing -> record {
    let source = open --raw $file
    if ($source | describe) != string {
        return { file: $file, unreadable: true, unverified: false, problems: [] }
    }
    let unverified = ($source | str length --utf-8-bytes | into filesize) > $max_size
    let problems = $formatters
        | each {|formatter|
              let reason = problem $file $source $formatter $oracle $limit --skip-ast=$unverified
              if $reason != null {
                  { formatter: $formatter.name, reason: $reason }
              }
          }
        | compact

    { file: $file, unreadable: false, unverified: $unverified, problems: $problems }
}

def problem [
    file: path
    source: string
    formatter: record
    oracle: path
    limit: duration
    --skip-ast
]: nothing -> oneof<string, nothing> {
    cd ($file | path dirname)
    let seconds = $limit / 1sec
    let formatted = $source
        | ^timeout $seconds ...(do $formatter.file $file)
        | complete
    match $formatted.exit_code {
        0 => {}
        124 => { return "times out" }
        $code if $code == $formatter.rejection_exit_code? and (nu-rejects $file) => {
            return "rejects a file nu rejects"
        }
        _ => { return fails }
    }
    if $skip_ast {
        return null
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

# Nu also rejects a file whose modules it cannot find, so SYNTAX_ERRORS in the corpus test pins the files nuten rejects.
# The corpus holds no config files, so nuten exits with its rejection code there only on a syntax error.
def nu-rejects [file: path]: nothing -> bool {
    let checked = with-env { FILE: $file } {
        ^$nu.current-exe --no-config-file --commands 'nu-check $env.FILE' | complete
    }

    ($checked.stdout | str trim) == "false"
}

def report []: list<record> -> record {
    let verdicts = $in

    {
        compared: ($verdicts | where not $it.unreadable and ($it.problems | is-empty) | get file)
        excluded: ($verdicts | excluded)
        unreadable: ($verdicts | where unreadable | get file)
        unverified: ($verdicts | where unverified | get file)
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
