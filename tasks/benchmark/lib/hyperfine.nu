export def medians [
    commands: list<record>
    runs: int
    options: list<string>
    subject: path
]: nothing -> record {
    let export = mktemp --tmpdir --suffix .json
    let named = $commands
        | each {|command| [--command-name, $command.name, (command-line $command.words)] }
        | flatten
    let settings = [--warmup, 1, --runs, $runs, --export-json, $export]
    let result = run-external hyperfine ...$settings ...$options ...$named | complete
    let exported = open --raw $export
    rm $export
    if $result.exit_code != 0 {
        error make {
            msg: $"hyperfine failed on ($subject) with exit code ($result.exit_code): ($result.stderr)"
        }
    }
    let results = $exported | from json | get results

    $results
    | first
    | get summary
    | columns
    | each {|metric| [$metric, ($results | medians-of $metric)] }
    | into record
}

def medians-of [metric: string]: list<record> -> record {
    each {|result| { name: $result.name, median: ($result.summary | get $metric | median) } }
    | transpose --header-row --as-record
}

def median []: record -> oneof<duration, filesize, int> {
    let summary = $in

    match $summary.unit? {
        second => ($summary.median * 1sec)
        byte => ($summary.median | into int | into filesize)
        _ => ($summary.median | into int)
    }
}

def command-line [words: list<string>]: nothing -> string {
    $words
    | each { $"'($in | str replace --all "'" "'\\''")'" }
    | str join ' '
}
