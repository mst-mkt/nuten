# Compares the work two nuten do on each input under valgrind, whose counts hold still between runs unlike time.

use cpus.nu any-allowed-cpu

const INPUTS = path self inputs.nuon
const DEPTH = 64
const METRICS = [instructions, allocations, allocated_bytes, peak_bytes]
# Code elsewhere can shift the counts a little through inlining, which is no regression.
const THRESHOLD_PERCENT = 1

const TIMED_OUT = {
    timed_out: true
    instructions: null
    allocations: null
    allocated_bytes: null
    peak_bytes: null
}

# Prints how the work of head differs from that of base on each input, and fails if it grows past the threshold or runs out of time.
def main [
    corpus: path # The corpus directory, as NUTEN_CORPUS names it
    base: path # The nuten binary to compare against
    head: path # The nuten binary to compare
    --time-limit: duration = 100sec # How long one run may take before it is cut off
]: nothing -> nothing {
    let missing = [valgrind, timeout, taskset] | where { which $in | is-empty }
    if not ($missing | is-empty) {
        print --stderr $"($missing | str join ', ') not found"
        exit 2
    }

    let inputs = inputs $corpus (open $INPUTS)
    let before = measure-each $base $inputs $time_limit
    let unmeasured = $before | where timed_out | get input
    if not ($unmeasured | is-empty) {
        print --stderr $"base timed out on ($unmeasured | str join ', ')"
        exit 2
    }
    let comparison = compare $before (measure-each $head $inputs $time_limit)

    print $comparison
    if ($comparison | any { $in.regressed }) {
        exit 1
    }
}

def measure-each [nuten: path, inputs: list<record>, limit: duration]: nothing -> table {
    $inputs | each {|input| { input: $input.name ...(measure $nuten $input $limit) } }
}

def compare [before: table, after: table]: nothing -> table {
    $before
    | zip $after
    | each {|pair| compare-input $pair.0 $pair.1 }
    | flatten
}

# A timeout has no counts to compare, and is a regression in itself.
def compare-input [before: record, after: record]: nothing -> list<record> {
    if $after.timed_out {
        return [
            {
                input: $after.input
                metric: timed_out
                base: null
                head: null
                change_percent: null
                regressed: true
            }
        ]
    }

    $METRICS
    | each {|metric|
          let base = $before | get $metric
          let head = $after | get $metric
          let change = ($head - $base) / $base * 100

          {
              input: $after.input
              metric: $metric
              base: $base
              head: $head
              change_percent: ($change | math round --precision 2)
              regressed: ($change > $THRESHOLD_PERCENT)
          }
      }
}

def inputs [corpus: path, picks: record]: nothing -> list<record> {
    [
        ...(file-inputs $corpus $picks)
        (directory-input $corpus $picks.directory)
        ...(nest-inputs $corpus)
    ]
}

def file-inputs [corpus: path, picks: record]: nothing -> list<record> {
    $picks
    | reject directory
    | values
    | uniq
    | each {|file|
          let path = $corpus | path join $file
          {
              name: $file
              arguments: [format --stdin-filepath $path -]
              stdin: (open --raw $path)
              exit_codes: [0]
          }
      }
}

# nuten checks with 1 when a file would change, which is no failure here.
def directory-input [corpus: path, directory: string]: nothing -> record {
    {
        name: $directory
        arguments: [format, --check, ($corpus | path join $directory)]
        stdin: null
        exit_codes: [0, 1]
    }
}

# Nests are made rather than picked, since the corpus nests too shallow to show a blowup.
# A path in the corpus keeps nuten from taking a config found from where this runs.
def nest-inputs [corpus: path]: nothing -> list<record> {
    let generated = $corpus | path join generated.nu

    [[name, open, close];
        [nested-parens, "(", ")"]
        [nested-lists, "[", "]"]
        [nested-records, "{a: ", "}"]
        [nested-closures, "{|| ", " }"]
    ]
    | each {|nest|
          {
              name: $nest.name
              arguments: [format --stdin-filepath $generated -]
              stdin: (nest $nest.open $nest.close $DEPTH)
              exit_codes: [0]
          }
      }
}

def nest [open: string, close: string, depth: int]: nothing -> string {
    let opens = 1..$depth | each { $open } | str join
    let closes = 1..$depth | each { $close } | str join

    $"($opens)1($closes)"
}

# DHAT is skipped after a timeout, which it would only reach again.
def measure [nuten: path, input: record, limit: duration]: nothing -> record {
    let cachegrind = run-valgrind [
        --tool=cachegrind
        --cache-sim=no
        --cachegrind-out-file=/dev/null
    ] $nuten $input $limit
    if $cachegrind == null {
        return $TIMED_OUT
    }
    let dhat = run-valgrind [--tool=dhat --dhat-out-file=/dev/null] $nuten $input $limit
    if $dhat == null {
        return $TIMED_OUT
    }

    { timed_out: false, instructions: ($cachegrind | instructions), ...($dhat | allocations) }
}

# The report goes to its own file, so what nuten writes to stderr does not mix in.
# One CPU leaves nuten one worker, so the counts do not hang on how its threads take turns.
def run-valgrind [
    tool: list<string>
    nuten: path
    input: record
    limit: duration
]: nothing -> oneof<string, nothing> {
    let log = mktemp --tmpdir
    let seconds = $limit / 1sec
    let cpu = any-allowed-cpu
    let result = $input.stdin
        | ^timeout $seconds taskset --cpu-list $cpu valgrind ...$tool $"--log-file=($log)" $nuten ...$input.arguments
        | complete
    let report = open --raw $log
    rm $log

    if $result.exit_code == 124 {
        return null
    }
    if ($report | is-empty) {
        error make { msg: $"valgrind did not run on ($input.name): ($result.stderr)" }
    }
    if $result.exit_code not-in $input.exit_codes {
        error make { msg: $"($input.name) failed under valgrind: ($result.stderr)" }
    }
    $report
}

# Valgrind groups the digits with commas, which into int does not read.
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
