use std/assert
use std/testing *

source benchmark-svg.nu

const SCRIPT = path self benchmark-svg.nu
const MIB = 1_048_576

@before-each
def setup []: nothing -> record {
    { dir: (mktemp --directory) }
}

@after-each
def teardown []: record -> nothing {
    rm --recursive $in.dir
}

def comparison []: nothing -> record {
    let directory = {
        time_wall_clock: { nuten: 50_000_000, nufmt: 125_000_000 }
        memory_peak_resident: { nuten: (20 * $MIB), nufmt: (50 * $MIB) }
    }

    {
        measured_at: "2026-10-18T02:50:32+09:00"
        versions: { nuten: 1177dc8, nufmt: { rev: ce67377fb9131218a9319e57ede252cec60bf033 } }
        environment: { cpu: "Intel(R) Core(TM)  Ultra 9 185H ", cpus: 22 }
        files: 1752
        excluded: [
            { formatter: nufmt, reason: "changes the AST", files: [a.nu, b.nu] }
            { formatter: nufmt, reason: fails, files: [c.nu] }
            { formatter: nuten, reason: "rejects a file nu rejects", files: [d.nu] }
        ]
        directories: [
            { name: nu-std, files: 40, all_cpus: $directory }
            { name: all, files: 1497, all_cpus: $directory }
        ]
        sizes: { nuten: (3 * $MIB), nufmt: (12 * $MIB) }
    }
}

def group [nuten: int, nufmt: int]: nothing -> record {
    { name: N, detail: d, nuten: $nuten, nufmt: $nufmt, unit: bytes, note: "4.0x smaller" }
}

def row [value: int]: nothing -> record {
    { name: nuten, value: $value, weight: bold, note: null }
}

def widths []: list<record> -> list<float> {
    where tag == rect | get attributes.width | into float
}

def texts []: list<record> -> list<string> {
    where tag == text | get content | each { where { ($in | describe) == string } | str join }
}

@test
def "main writes charts for light and dark themes" [] {
    let context = $in
    let input = $context.dir | path join compare.json
    let out = $context.dir | path join out
    comparison | to json | save $input

    ^$nu.current-exe --no-config-file $SCRIPT $input $out

    assert equal (ls $out | get name | path basename | sort) [readme-dark.svg readme-light.svg]
    assert str contains (open --raw ($out | path join readme-dark.svg)) $THEMES.dark.nuten
    assert str contains (open --raw ($out | path join readme-light.svg)) $THEMES.light.nuten
}

@test
def "groups gives speed, memory, binary size, and correctness with notes" [] {
    let comparison = comparison

    let actual = $comparison | groups

    assert equal ($actual | select name nuten nufmt unit note) [
        { name: Speed nuten: 50_000_000 nufmt: 125_000_000 unit: nanoseconds note: "2.5x faster" }
        { name: Memory nuten: (20 * $MIB) nufmt: (50 * $MIB) unit: bytes note: "2.5x less" }
        { name: "Binary size" nuten: (3 * $MIB) nufmt: (12 * $MIB) unit: bytes note: "4.0x smaller" }
        { name: Correctness nuten: 0 nufmt: 2 unit: files note: "keeps every AST" }
    ]
}

@test
def "groups tells files compared and files in corpus" [] {
    let comparison = comparison

    let actual = $comparison | groups | get detail

    assert str contains $actual.0 "1,497 files"
    assert str contains $actual.3 "out of 1,752 files in the corpus"
}

@test
def "all-directory names missing directory in error" [] {
    let comparison = comparison | update directories { where name != all }

    let found = try { $comparison | all-directory } catch {|error| $error.msg }

    assert str contains $found "no directory named all"
}

@test
def "correctness fails where nuten fails on file" [] {
    let comparison = comparison
        | update excluded { append { formatter: nuten, reason: fails, files: [e.nu] } }

    let counted = try { $comparison | correctness } catch {|error| $error.msg }

    assert str contains $counted "nuten fails"
}

@test
def "correctness counts files where nuten changes ast" [] {
    let comparison = comparison
        | update excluded { append { formatter: nuten, reason: "changes the AST", files: [e.nu] } }

    let actual = $comparison | correctness

    assert equal $actual.nuten 1
}

@test
def "with-note fails where nuten equals nufmt" [] {
    let group = { name: N, nuten: 5, nufmt: 5, comparative: faster }

    let noted = { $group | with-note }

    assert error $noted
}

@test
def "with-note fails where nuten is behind nufmt" [] {
    let group = { name: N, nuten: 6, nufmt: 5, comparative: faster }

    let noted = { $group | with-note }

    assert error $noted
}

@test
def "with-note fails where lead of nuten rounds to one" [] {
    let group = { name: N, nuten: 100, nufmt: 104, comparative: faster }

    let noted = { $group | with-note }

    assert error $noted
}

@test
def "amount gives time in whole milliseconds from 10 ms" [] {
    let times = [56_883_288, 10_000_000, 9_960_000]

    let actual = $times | each { amount nanoseconds }

    assert equal $actual ["57 ms" "10 ms" "10 ms"]
}

@test
def "amount gives time below 10 ms to tenth of millisecond" [] {
    let time = 5_075_945

    let actual = $time | amount nanoseconds

    assert equal $actual "5.1 ms"
}

@test
def "amount gives bytes in mebibytes to one place" [] {
    let bytes = 25_550_848

    let actual = $bytes | amount bytes

    assert equal $actual "24.4 MiB"
}

@test
def "amount gives count of files" [] {
    let counts = [0, 1, 1254]

    let actual = $counts | each { amount files }

    assert equal $actual ["0 files" "1 file" "1,254 files"]
}

@test
def "group-digits puts comma between every three digits whatever locale" [] {
    # Nushell reads the locale when it starts, so the numbers are grouped in a process of its own.
    let script = $"source ($SCRIPT | to nuon); [7, 1752, 1234567] | each { group-digits } | to nuon"

    let actual = with-env { LC_ALL: de_DE.UTF-8 } { ^$nu.current-exe --no-config-file --commands $script }

    assert equal ($actual | from nuon) ["7" "1,752" "1,234,567"]
}

@test
def "row-nodes keeps tiny value visible" [] {
    let row = row 1

    let actual = $row | row-nodes 0 100_000 bytes $THEMES.light

    assert equal ($actual | widths | last) ($MIN_BAR_WIDTH | into float)
}

@test
def "row-nodes draws no bar for zero" [] {
    let row = row 0

    let actual = $row | row-nodes 0 4 files $THEMES.light

    assert equal ($actual | widths | last) 0.0
}

@test
def "note-node writes note after bar where gap fits it" [] {
    let note = "2.5x faster"
    let filled = $TRACK_WIDTH - (($note | str length) * $NOTE_CHAR_WIDTH + 2 * $NOTE_PADDING)

    let actual = $note | note-node 20 $filled $THEMES.light

    assert equal $actual.attributes.fill $THEMES.light.nuten
    assert equal $actual.attributes.x ($LABEL_WIDTH + $filled + $NOTE_PADDING | into string)
}

@test
def "note-node writes note inside bar where gap falls short of it" [] {
    let note = "2.5x faster"
    let filled = $TRACK_WIDTH - (($note | str length) * $NOTE_CHAR_WIDTH + 2 * $NOTE_PADDING) + 1

    let actual = $note | note-node 20 $filled $THEMES.light

    assert equal $actual.attributes.fill $THEMES.light.on_nuten
    assert equal $actual.attributes.text-anchor end
}

@test
def "group-nodes fills nufmt bar to whole track" [] {
    let group = group 1 4

    let actual = $group | group-nodes 0 $THEMES.light

    assert equal ($actual | widths) (
        [$TRACK_WIDTH, ($TRACK_WIDTH / 4), $TRACK_WIDTH, $TRACK_WIDTH] | into float
    )
}

@test
def "group-nodes writes note on nuten row alone" [] {
    let group = group 1 4

    let actual = $group | group-nodes 0 $THEMES.light

    assert equal ($actual | texts | where $it == "4.0x smaller" | length) 1
}

@test
def "draw stacks groups with gap between and footer below" [] {
    let groups = [(group 1 2), (group 1 2), (group 1 2)]
    let groups_height = 3 * $GROUP_HEIGHT + 2 * $GROUP_GAP

    let actual = $groups | draw $THEMES.light [f] | from xml

    let headings = $actual.content | where tag == text and ("tspan" in $it.content.tag)
    assert equal ($headings | get attributes.y | into int) (
        [0, 1, 2] | each { $in * ($GROUP_HEIGHT + $GROUP_GAP) + $HEADING_HEIGHT - $DESCENT }
    )
    assert equal ($actual.attributes.height | into int) (
        $groups_height + $FOOTER_GAP + $FOOTER_LINE_HEIGHT
    )
}

@test
def "draw titles chart with values of each formatter" [] {
    let groups = [(group $MIB (2 * $MIB))]

    let actual = $groups | draw $THEMES.light [f] | from xml

    assert equal ($actual.content | where tag == title | get 0.content.0.content) (
        "N, d: nuten 1.0 MiB, nufmt 2.0 MiB"
    )
}

@test
def "footer tells files compared, date in utc, versions, and machine" [] {
    let comparison = comparison

    let actual = $comparison | footer

    assert equal $actual [
        "Speed and memory are measured on the 1,497 corpus files that both format without failing or changing the AST."
        "Measured on 2026-10-17 with nuten 1177dc8 and nufmt ce67377 on Intel Core Ultra 9 185H (22 threads)."
    ]
}

@test
def "footer-nodes breaks sentence one character longer than line" [] {
    let sentence = $"('' | fill --character a --width ($FOOTER_LINE_CHARS - 1)) b"

    let actual = [$sentence] | footer-nodes 0 $THEMES.light

    assert equal ($actual | length) 2
}

@test
def "wrap breaks sentence between words within width and keeps long word whole" [] {
    let sentence = "aa bb longword cc"

    let actual = $sentence | wrap 5

    assert equal $actual ["aa bb" longword cc]
}
