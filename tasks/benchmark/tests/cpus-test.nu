use std/assert
use std/testing *

use ../cpus.nu [allowed-cpus any-allowed-cpu parse-cpu-list]

const CPUS = path self ../cpus.nu

@test
def "allowed-cpus gives cpus process may run on" [] {
    let cpu = any-allowed-cpu
    let script = $"use ($CPUS | to nuon) allowed-cpus; allowed-cpus | to nuon"

    let actual = ^taskset --cpu-list $cpu $nu.current-exe --no-config-file --commands $script
    | from nuon

    assert equal $actual [$cpu]
}

@test
def "parse-cpu-list gives each cpu in ranges and single cpus" [] {
    let actual = "0-3,5,8-9" | parse-cpu-list

    assert equal $actual [0 1 2 3 5 8 9]
}

@test
def "any-allowed-cpu gives cpu taskset runs on" [] {
    let actual = any-allowed-cpu

    let run = ^taskset --cpu-list $actual true | complete
    assert equal $run.exit_code 0 $run.stderr
}
