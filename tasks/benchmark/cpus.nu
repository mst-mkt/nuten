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

# The cores of a hybrid CPU differ in speed, so the fastest is taken, and the same one every time.
export def fastest-allowed-cpu []: nothing -> int {
    let frequencies = allowed-cpus | each {|cpu| { cpu: $cpu frequency: (max-frequency $cpu) } }
    let fastest = $frequencies | get frequency | math max

    $frequencies | where frequency == $fastest | first | get cpu
}

# A machine without cpufreq, as some virtual machines are, tells no frequency, so every CPU counts the same there.
def max-frequency [cpu: int]: nothing -> int {
    let path = $"/sys/devices/system/cpu/cpu($cpu)/cpufreq/cpuinfo_max_freq"
    if ($path | path exists) {
        open --raw $path | str trim | into int
    } else {
        0
    }
}
