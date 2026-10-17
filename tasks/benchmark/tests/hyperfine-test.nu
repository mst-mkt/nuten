use std/assert
use std/testing *

use ../lib/cpus.nu allowed-cpus
source ../lib/hyperfine.nu

@before-each
def setup []: nothing -> record {
    { dir: (mktemp --directory) }
}

@after-each
def teardown []: record -> nothing {
    rm --recursive $in.dir
}

@test
def "medians gives median of each metric hyperfine measured for each command" [] {
    let context = $in
    let commands = [[name, words]; [a, ['true']], [b, ['true']]]

    let actual = medians $commands 1 [] $context.dir

    assert ([time_wall_clock, memory_peak_resident] | all { $in in ($actual | columns) })
    assert equal ($actual.time_wall_clock | columns) [a b]
    assert ($actual.time_wall_clock.a > 0sec)
    assert ($actual.memory_peak_resident.b > 0b)
}

@test
def "medians runs commands on cpu given" [] {
    let context = $in
    let ran = $context.dir | path join ran
    let commands = [[name, words];
        [a, [sh, -c, $"grep Cpus_allowed_list /proc/self/status > ($ran)"]]
    ]
    let cpu = allowed-cpus | last

    medians $commands 1 [] $context.dir --cpu $cpu

    assert equal (open $ran | str trim) $"Cpus_allowed_list:\t($cpu)"
}

@test
def "medians runs each command once to warm up and then as many times as asked" [] {
    let context = $in
    let commands = [[name, words]; [a, [sh, -c, "echo >> ran"]]]

    do {
        cd $context.dir
        medians $commands 2 [] $context.dir
    }

    assert equal (open ($context.dir | path join ran) | lines | length) 3
}

@test
def "medians gives median of runs" [] {
    let context = $in
    # The three runs after the warmup sleep 0.1, 2, and 0.4 seconds, so only their median lies in [0.4, 0.8).
    let script = "echo >> ran; case $(wc -l < ran) in 2) sleep 0.1 ;; 3) sleep 2 ;; *) sleep 0.4 ;; esac"
    let commands = [[name, words]; [a, [sh, -c, $script]]]

    let actual = do {
        cd $context.dir
        medians $commands 3 [] $context.dir
    }

    assert ($actual.time_wall_clock.a >= 400ms)
    assert ($actual.time_wall_clock.a < 800ms)
}

@test
def "medians fails naming subject when command fails" [] {
    let context = $in
    let commands = [[name, words]; [a, ['false']]]

    let actual = try { medians $commands 1 [] $context.dir } catch {|error| $error.msg }

    assert str contains $actual $"hyperfine failed on ($context.dir)"
}

@test
def "median reads seconds as duration, bytes as filesize, and counts as int" [] {
    let summaries = [
        { unit: second, median: 0.25 }
        { unit: byte, median: 1024.0 }
        { median: 9.0 }
    ]

    let actual = $summaries | each { median }

    assert equal $actual [250ms 1024b 9]
}

@test
def "command-line quotes each word" [] {
    let words = ["/a b/nuten", --stdin-filepath, "/c d/it's.nu"]

    let actual = command-line $words

    assert equal $actual "'/a b/nuten' '--stdin-filepath' '/c d/it'\\''s.nu'"
}
