use std/assert
use std/testing *

source ../lib/valgrind.nu

@before-each
def setup []: nothing -> record {
    { dir: (mktemp --directory) }
}

@after-each
def teardown []: record -> nothing {
    rm --recursive $in.dir
}

def command [
    words: list<string>
    --stdin: string
    --exit-codes: list<int> = [0]
]: nothing -> record {
    { name: test, words: $words, stdin: $stdin, exit_codes: $exit_codes }
}

@test
def "count-instructions counts instructions of run" [] {
    let actual = count-instructions (command [cat] --stdin "ls") 100sec

    assert ($actual > 0)
}

@test
def "count-instructions counts more instructions for more work" [] {
    let short = count-instructions (command [sh, -c, "true"]) 100sec
    let long = count-instructions (
        command [sh, -c, "i=0; while [ $i -lt 1000 ]; do i=$((i + 1)); done"]
    ) 100sec

    assert ($long > $short)
}

@test
def "count-instructions passes stdin to run" [] {
    let context = $in
    let got = $context.dir | path join got

    count-instructions (command [sh, -c, $"cat > ($got)"] --stdin "ls | length") 100sec

    assert equal (open --raw $got) "ls | length"
}

@test
def "count-instructions gives nothing for run past time limit" [] {
    let actual = count-instructions (command [sleep, 10]) 1sec

    assert equal $actual null
}

@test
def "count-instructions takes exit code run allows" [] {
    let actual = count-instructions (command [sh, -c, "exit 1"] --exit-codes [0, 1]) 100sec

    assert ($actual > 0)
}

@test
def "count-instructions fails naming run that exits with code it does not allow" [] {
    let actual = try {
        count-instructions (command [sh, -c, "exit 1"]) 100sec
    } catch {|error|
        $error.msg
    }

    assert str contains $actual "test failed under valgrind"
}

@test
def "count-instructions fails naming run valgrind cannot start" [] {
    let actual = try {
        count-instructions (command [/nonexistent] --exit-codes [127]) 100sec
    } catch {|error|
        $error.msg
    }

    assert str contains $actual "valgrind did not run on test"
}

@test
def "count-instructions runs run on one cpu" [] {
    let context = $in
    let got = $context.dir | path join got

    count-instructions (
        command [sh, -c, $"grep Cpus_allowed_list /proc/self/status > ($got)"]
    ) 100sec

    assert ((open $got | str trim) =~ '^Cpus_allowed_list:\s+\d+$')
}

@test
def "count-allocations counts allocations of run" [] {
    let actual = count-allocations (command [cat] --stdin "ls") 100sec

    assert ($actual.allocations > 0)
    assert ($actual.allocated_bytes >= $actual.peak_bytes)
}

@test
def "count-allocations gives nothing for run past time limit" [] {
    let actual = count-allocations (command [sleep, 10]) 1sec

    assert equal $actual null
}

@test
def "instructions reads count from cachegrind report" [] {
    let report = "==1== \n==1== I refs:        345,770,384\n"

    let actual = $report | instructions

    assert equal $actual 345770384
}

@test
def "allocations reads blocks and bytes from dhat report" [] {
    let report = [
        "==1== Total:     22,178 bytes in 147 blocks"
        "==1== At t-gmax: 11,274 bytes in 23 blocks"
        "==1== At t-end:  8,736 bytes in 2 blocks"
    ]
        | str join "\n"

    let actual = $report | allocations

    assert equal $actual { allocations: 147 allocated_bytes: 22178 peak_bytes: 11274 }
}
