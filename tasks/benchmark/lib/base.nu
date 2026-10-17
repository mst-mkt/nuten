use valgrind.nu [count-allocations, count-instructions]

const INPUTS = path self ../inputs.nuon
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

export def changes [corpus: path, base: path, head: path, time_limit: duration]: nothing -> record {
    let inputs = inputs $corpus (open $INPUTS)
    let before = measure-each $base $inputs $time_limit
    let base_timed_out = $before | where timed_out | get input
    if not ($base_timed_out | is-empty) {
        return { base_timed_out: $base_timed_out, changes: null }
    }

    { base_timed_out: [], changes: (compare $before (measure-each $head $inputs $time_limit)) }
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

def directory-input [corpus: path, directory: string]: nothing -> record {
    {
        name: $directory
        arguments: [format, --list-different, ($corpus | path join $directory)]
        stdin: null
        exit_codes: [0, 1]
    }
}

# Nests are made rather than picked, since the corpus nests too shallowly to show a blowup.
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

def measure [nuten: path, input: record, limit: duration]: nothing -> record {
    let run = {
        name: $input.name
        words: [$nuten, ...$input.arguments]
        stdin: $input.stdin
        exit_codes: $input.exit_codes
    }
    let instructions = count-instructions $run $limit
    if $instructions == null {
        return $TIMED_OUT
    }
    let allocations = count-allocations $run $limit
    if $allocations == null {
        return $TIMED_OUT
    }

    { timed_out: false, instructions: $instructions, ...$allocations }
}
