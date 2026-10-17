# CI runners have no hardware counters, and valgrind counts the same on any machine.

use cpus.nu any-allowed-cpu

export def count-instructions [run: record, limit: duration]: nothing -> oneof<int, nothing> {
    let report = run-valgrind [--tool=cachegrind --cache-sim=no --cachegrind-out-file=/dev/null] $run $limit
    if $report == null {
        return null
    }

    $report | instructions
}

export def count-allocations [run: record, limit: duration]: nothing -> oneof<record, nothing> {
    let report = run-valgrind [--tool=dhat --dhat-out-file=/dev/null] $run $limit
    if $report == null {
        return null
    }

    $report | allocations
}

# One CPU leaves the run one worker, so the counts do not depend on how its threads take turns.
def run-valgrind [
    tool: list<string>
    run: record
    limit: duration
]: nothing -> oneof<string, nothing> {
    let log = mktemp --tmpdir
    let seconds = $limit / 1sec
    let cpu = any-allowed-cpu
    let result = $run.stdin
        | ^timeout $seconds taskset --cpu-list $cpu valgrind ...$tool $"--log-file=($log)" ...$run.words
        | complete
    let report = open --raw $log
    rm $log

    if $result.exit_code == 124 {
        return null
    }
    if ($report | is-empty) {
        error make { msg: $"valgrind did not run on ($run.name): ($result.stderr)" }
    }
    if $result.exit_code not-in $run.exit_codes {
        error make { msg: $"($run.name) failed under valgrind: ($result.stderr)" }
    }
    $report
}

def instructions []: string -> int {
    str replace --all ',' ''
    | parse --regex 'I refs:\s+(?<count>\d+)'
    | get 0.count
    | into int
}

def allocations []: string -> record {
    let report = $in | str replace --all ',' ''
    let total = $report
        | parse --regex 'Total:\s+(?<bytes>\d+) bytes in (?<blocks>\d+) blocks'
        | first
        | into int bytes blocks
    let peak = $report | parse --regex 'At t-gmax:\s+(?<bytes>\d+) bytes' | first | into int bytes

    { allocations: $total.blocks, allocated_bytes: $total.bytes, peak_bytes: $peak.bytes }
}
