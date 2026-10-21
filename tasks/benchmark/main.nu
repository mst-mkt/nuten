use lib/base.nu
use lib/cpus.nu fastest-allowed-cpu
use lib/nufmt.nu
use lib/select.nu

# The benchmarks of nuten, which run it on files from the corpus.
def main []: nothing -> nothing {
    print (help main)
}

# How the work of head differs from that of base on each input.
def "main base" [
    corpus: path # The corpus directory, as NUTEN_CORPUS names it
    base: path # The nuten binary to compare against
    head: path # The nuten binary to compare
    --time-limit: duration = 100sec # How long one run may take before it is cut off
]: nothing -> table {
    require [valgrind, timeout, taskset]
    let result = base changes $corpus $base $head $time_limit
    if not ($result.base_timed_out | is-empty) {
        stop $"base timed out on ($result.base_timed_out | str join ', ')"
    }

    $result.changes
}

# How nuten and nufmt fare on the files both format without breaking, with what was left out and why.
def "main nufmt" [
    corpus: path # The corpus directory, as NUTEN_CORPUS names it
    nuten: path # The nuten binary
    nufmt: path # The nufmt binary
    oracle: path # The nuten-oracle binary, which asks nu whether formatting keeps the AST
    ...also: path # More files to compare, such as the one nufmt benchmarks itself on
    --time-limit: duration = 100sec # How long a formatter may take on one file before it is cut off, a hundredfold under valgrind
    --max-size: filesize = 512KiB # How large a file may be for the oracle to check that formatting keeps its AST
    --runs: int = 5 # How many times each formatter is timed on each file
]: nothing -> record {
    if $runs < 1 {
        stop "--runs must be at least 1"
    }
    require [timeout, hyperfine, valgrind, taskset]

    nufmt compare $corpus $nuten $nufmt $oracle $also $time_limit $max_size $runs (
        fastest-allowed-cpu
    )
}

# The picked inputs of base as NUON, with paths relative to the corpus.
def "main select" [
    corpus: path # The corpus directory, as NUTEN_CORPUS names it
    nuten: path # The nuten binary, which must format each picked file
]: nothing -> string {
    if (glob ($corpus | path join '**/*.nu') | is-empty) {
        stop $"no .nu file under ($corpus)"
    }
    let picks = select picks $corpus $nuten
    let unpicked = $picks | transpose rule path | where path == null | get rule
    if not ($unpicked | is-empty) {
        stop $"no file for ($unpicked | str join ', ') under ($corpus)"
    }

    $picks | select render $nuten
}

def require [tools: list<string>]: nothing -> nothing {
    let missing = $tools | where { which $in | is-empty }
    if not ($missing | is-empty) {
        stop $"($missing | str join ', ') not found"
    }
}

def stop [message: string]: nothing -> nothing {
    print --stderr $message
    exit 2
}
