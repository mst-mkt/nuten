export def allowed-cpus []: nothing -> list<int> {
    open /proc/self/status
    | lines
    | where $it starts-with Cpus_allowed_list
    | first
    | split row ':'
    | get 1
    | str trim
    | parse-cpu-list
}

export def parse-cpu-list []: string -> list<int> {
    split row ','
    | each {|range|
          let bounds = $range | split row '-' | into int
          seq $bounds.0 ($bounds | last)
      }
    | flatten
}

# Any one does, and picking at random keeps runs side by side off the same CPU.
export def any-allowed-cpu []: nothing -> int {
    allowed-cpus | shuffle | first
}
