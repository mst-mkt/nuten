use ../tasks/benchmark/lib/base.nu

# The body of a pull request comment telling how the work of head differs from that of base.
def main [
    corpus: path # The corpus directory, as NUTEN_CORPUS names it
    base: path # The nuten binary of the base branch
    head: path # The nuten binary of the pull request
    --time-limit: duration = 100sec # How long one run may take before it is cut off
]: nothing -> string {
    let result = base changes $corpus $base $head $time_limit
    if not ($result.base_timed_out | is-empty) {
        error make { msg: $"base timed out on ($result.base_timed_out | str join ', ')" }
    }

    $result.changes | comment
}

def comment []: table -> string {
    let comparison = $in
    let regressed = $comparison | where regressed
    let input_count = $comparison | get input | uniq | length

    let summary = if ($regressed | is-empty) {
        $"✅ None of the ($input_count) inputs regressed."
    } else {
        $"⚠️ ($regressed | get input | uniq | length) of the ($input_count) inputs regressed past the threshold."
    }
    let regressions = if ($regressed | is-empty) {
        []
    } else {
        ["### Regressions", ($regressed | measures | to md)]
    }
    let counts = $comparison
        | where metric != 'timed_out'
        | group-by --to-table metric
        | each {|group| $"#### ($group.metric)\n\n($group.items | measures | reject metric | to md)" }

    [
        "## Benchmark"
        $summary
        ...$regressions
        "### Changes"
        ($comparison | by-input | to md)
        "<details><summary>Counts</summary>"
        ...$counts
        "</details>"
    ]
    | str join "\n\n"
}

def by-input []: table -> table {
    let comparison = $in
    let metrics = $comparison | where metric != 'timed_out' | get metric | uniq

    $comparison
    | group-by --to-table input
    | each {|group|
          let cells = if 'timed_out' in $group.items.metric {
              $metrics | each { [$in, "timed out"] }
          } else {
              $group.items | each { [$in.metric, ($in | change)] }
          }
          { "": ($group.items | status), input: $group.input, ...($cells | into record) }
      }
}

def measures []: table -> table {
    each {|row|
        {
            "": ([$row] | status)
            input: $row.input
            metric: $row.metric
            base: ($row.base | group-digits)
            head: ($row.head | group-digits)
            change: ($row | change)
        }
    }
}

def status []: table -> string {
    if 'timed_out' in $in.metric { "⏱️" } else if true in $in.regressed { "🔺" } else { "✅" }
}

def change []: record -> string {
    let row = $in
    match $row.change_percent {
        null => "timed out"
        $percent if $row.regressed => $"**($percent | percent)**"
        $percent => ($percent | percent)
    }
}

def percent []: float -> string {
    match $in {
        0.0 => "0%"
        $percent if $percent > 0 => $"+($percent)%"
        $percent => $"($percent)%"
    }
}

def group-digits []: oneof<int, nothing> -> string {
    if $in == null { "" } else { into string --group-digits }
}
