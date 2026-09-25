# Measures the work nuten does on each input under valgrind, whose counts hold still between runs unlike time.

const INPUTS = path self inputs.nuon
const DEPTH = 64
const TIME_LIMIT = 100sec

const TIMED_OUT = {
    timed_out: true
    instructions: null
    allocations: null
    allocated_bytes: null
    peak_bytes: null
}

# Prints the instructions and allocations nuten takes on each input.
def main [
    corpus: path # The corpus directory, as NUTEN_CORPUS names it
    nuten: path # The nuten binary to measure
]: nothing -> table {
    let missing = [valgrind timeout taskset] | where { which $in | is-empty }
    if not ($missing | is-empty) {
        print --stderr $"($missing | str join ', ') not found"
        exit 2
    }

    inputs $corpus (open $INPUTS)
    | each {|input| { input: $input.name ...(measure $nuten $input $TIME_LIMIT) } }
}

def inputs [corpus: path picks: record]: nothing -> list<record> {
    [
        ...(file-inputs $corpus $picks)
        (directory-input $corpus $picks.directory)
        ...(nest-inputs $corpus)
    ]
}

def file-inputs [corpus: path picks: record]: nothing -> list<record> {
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
def directory-input [corpus: path directory: string]: nothing -> record {
    {
        name: $directory
        arguments: [format --check ($corpus | path join $directory)]
        stdin: null
        exit_codes: [0 1]
    }
}

# Nests are made rather than picked, since the corpus nests too shallow to show a blowup.
# A path in the corpus keeps nuten from taking a config found from where this runs.
def nest-inputs [corpus: path]: nothing -> list<record> {
    let generated = $corpus | path join generated.nu

    [[name open close];
        [nested-parens "(" ")"]
        [nested-lists "[" "]"]
        [nested-records "{a: " "}"]
        [nested-closures "{|| " " }"]
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

def nest [open: string close: string depth: int]: nothing -> string {
    let opens = 1..$depth | each { $open } | str join
    let closes = 1..$depth | each { $close } | str join

    $"($opens)1($closes)"
}

# DHAT is skipped after a timeout, which it would only reach again.
def measure [nuten: path input: record limit: duration]: nothing -> record {
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

    { timed_out: false instructions: ($cachegrind | instructions) ...($dhat | allocations) }
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

# Any one does, and picking at random keeps runs side by side off the same CPU.
def any-allowed-cpu []: nothing -> int {
    open /proc/self/status
    | lines
    | where $it starts-with Cpus_allowed_list
    | first
    | split row ':'
    | get 1
    | str trim
    | split row ','
    | each {|range|
          let bounds = $range | split row '-' | into int
          seq $bounds.0 ($bounds | last)
      }
    | flatten
    | shuffle
    | first
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

    { allocations: $total.blocks allocated_bytes: $total.bytes peak_bytes: $peak.bytes }
}
