const WIDTH = 720
const LABEL_WIDTH = 64
const VALUE_WIDTH = 100
const VALUE_GAP = 12
const TRACK_WIDTH = $WIDTH - $LABEL_WIDTH - $VALUE_WIDTH
const HEADING_HEIGHT = 28
const ROW_HEIGHT = 30
const GROUP_HEIGHT = $HEADING_HEIGHT + 2 * $ROW_HEIGHT
const GROUP_GAP = 18
const BAR_HEIGHT = 18
const MIN_BAR_WIDTH = 2
const DESCENT = 10
const NOTE_CHAR_WIDTH = 8
const FOOTER_CHAR_WIDTH = 6.5
const NOTE_PADDING = 10
const FOOTER_GAP = 14
const FOOTER_LINE_HEIGHT = 18
const FOOTER_DESCENT = 5
const FOOTER_LINE_CHARS = $WIDTH / $FOOTER_CHAR_WIDTH | math floor
const FONT = "-apple-system, BlinkMacSystemFont, 'Segoe UI', 'Noto Sans', Helvetica, Arial, sans-serif"
const MONO = "ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, monospace"

const THEMES = {
    light: {
        text: "#1f2328"
        muted: "#59636e"
        track: "#eff2f5"
        nuten: "#1f7f6b"
        on_nuten: "#ffffff"
        nufmt: "#818b98"
    }
    dark: {
        text: "#e6edf3"
        muted: "#9198a1"
        track: "#21262d"
        nuten: "#5fcdb1"
        on_nuten: "#0d1117"
        nufmt: "#6e7681"
    }
}

# The README charts of the nufmt comparison, written as readme-light.svg and readme-dark.svg into out.
def main [
    comparison: path # The JSON that tasks/benchmark/main.nu nufmt wrote
    out: path # The directory to write into
]: nothing -> nothing {
    let comparison = open $comparison
    let groups = $comparison | groups
    let footer = $comparison | footer
    mkdir $out

    $THEMES
    | transpose name colors
    | each {|theme|
          $groups
          | draw $theme.colors $footer
          | save --force ($out | path join $"readme-($theme.name).svg")
      }
    | ignore
}

def all-directory []: record -> record {
    let directory = $in.directories | where name == all | get 0?
    if $directory == null {
        error make { msg: "the comparison times no directory named all" }
    }

    $directory
}

def groups []: record -> list<record> {
    let comparison = $in
    let directory = $comparison | all-directory
    let compared = $directory.files | group-digits

    [
        {
            name: Speed
            detail: $"time to format ($compared) files"
            nuten: $directory.all_cpus.time_wall_clock.nuten
            nufmt: $directory.all_cpus.time_wall_clock.nufmt
            unit: nanoseconds
            comparative: faster
        }
        {
            name: Memory
            detail: $"peak usage while formatting ($compared) files"
            nuten: $directory.all_cpus.memory_peak_resident.nuten
            nufmt: $directory.all_cpus.memory_peak_resident.nufmt
            unit: bytes
            comparative: less
        }
        {
            name: "Binary size"
            detail: "size of the executable"
            nuten: $comparison.sizes.nuten
            nufmt: $comparison.sizes.nufmt
            unit: bytes
            comparative: smaller
        }
        ($comparison | correctness)
    ]
    | each { with-note }
}

# No AST is checked on a file nuten fails or times out on, so "keeps every AST" would claim more than was checked.
def correctness []: record -> record {
    let comparison = $in
    let excluded = $comparison.excluded
    let unchecked = $excluded
        | where formatter == nuten and reason not-in [
              "changes the AST"
              "rejects a file nu rejects"
          ]
    if not ($unchecked | is-empty) {
        error make { msg: $"nuten ($unchecked | get reason | str join ', ') on some files" }
    }
    let changes = {|formatter|
        $excluded
        | where formatter == $formatter and reason == "changes the AST"
        | get files
        | flatten
        | length
    }

    {
        name: Correctness
        detail: $"files where formatting changed the AST, out of ($comparison.files | group-digits) files in the corpus"
        nuten: (do $changes nuten)
        nufmt: (do $changes nufmt)
        unit: files
        comparative: fewer
        zero_note: "keeps every AST"
    }
}

# The note claims a lead, so a group without a visible one stops the chart instead of mislabeling it.
def with-note []: record -> record {
    let group = $in
    let behind = { error make { msg: $"nuten is not visibly ahead of nufmt in ($group.name)" } }
    if $group.nuten >= $group.nufmt {
        do $behind
    }
    let note = if $group.nuten == 0 {
        $group.zero_note
    } else {
        $"($group.nufmt / $group.nuten | fixed 1)x ($group.comparative)"
    }
    if ($note | str starts-with "1.0x") {
        do $behind
    }

    $group | reject comparative zero_note? | insert note $note
}

def footer []: record -> list<string> {
    let comparison = $in
    let compared = $comparison | all-directory | get files | group-digits
    let date = $comparison.measured_at
        | into datetime
        | date to-timezone UTC
        | format date "%Y-%m-%d"
    let cpu = $comparison.environment.cpu
        | str replace --all --regex '(?i)\((r|tm)\)|[®™]' ''
        | str replace --all --regex '\s+' ' '
        | str trim
    let nufmt_rev = $comparison.versions.nufmt.rev | str substring 0..6

    [
        $"Speed and memory are measured on the ($compared) corpus files that both format without failing or changing the AST."
        $"Measured on ($date) with nuten ($comparison.versions.nuten) and nufmt ($nufmt_rev) on ($cpu) \(($comparison.environment.cpus) threads\)."
    ]
}

def draw [colors: record, footer: list<string>]: list<record> -> string {
    let groups = $in
    let groups_height = ($groups | length) * ($GROUP_HEIGHT + $GROUP_GAP) - $GROUP_GAP
    let footer_lines = $footer | footer-nodes ($groups_height + $FOOTER_GAP) $colors
    let height = $groups_height + $FOOTER_GAP + ($footer_lines | length) * $FOOTER_LINE_HEIGHT
    let body = $groups
        | enumerate
        | each {|entry|
              $entry.item | group-nodes ($entry.index * ($GROUP_HEIGHT + $GROUP_GAP)) $colors
          }
        | flatten
    let attributes = {
        xmlns: "http://www.w3.org/2000/svg"
        width: $WIDTH
        height: $height
        viewBox: $"0 0 ($WIDTH) ($height)"
        role: img
    }

    node svg $attributes [(node title {} [(description $groups)]), ...$body, ...$footer_lines]
    | to xml --indent 2 --self-closed --partial-escape
    | $in + "\n"
}

def footer-nodes [top: int, colors: record]: list<string> -> list<record> {
    each { wrap $FOOTER_LINE_CHARS }
    | flatten
    | enumerate
    | each {|line|
          let baseline = $top + ($line.index + 1) * $FOOTER_LINE_HEIGHT - $FOOTER_DESCENT
          node text { x: 0, y: $baseline, fill: $colors.muted, font-size: 12, font-family: $FONT } [
              $line.item
          ]
      }
}

def wrap [width: int]: string -> list<string> {
    split row ' '
    | reduce --fold [] {|word, lines|
          if ($lines | is-empty) {
              [$word]
          } else if ($lines | last | str length) + 1 + ($word | str length) <= $width {
              $lines | drop | append $"($lines | last) ($word)"
          } else {
              $lines | append $word
          }
      }
}

def group-nodes [top: int, colors: record]: record -> list<record> {
    let group = $in
    let rows = [
        { name: nuten, value: $group.nuten, weight: bold, note: $group.note }
        { name: nufmt, value: $group.nufmt, weight: normal, note: null }
    ]
    # Indenting puts a newline between the tspans, which renders as the space after the name.
    let heading = node text {
        x: 0
        y: ($top + $HEADING_HEIGHT - $DESCENT)
        font-size: 14
        font-family: $FONT
    } [
        (node tspan { font-weight: bold, fill: $colors.text } [$group.name])
        (node tspan { fill: $colors.muted } [$"· ($group.detail)"])
    ]

    $rows
    | enumerate
    | each {|entry|
          let row_top = $top + $HEADING_HEIGHT + $entry.index * $ROW_HEIGHT
          $entry.item | row-nodes $row_top $group.nufmt $group.unit $colors
      }
    | flatten
    | prepend $heading
}

def row-nodes [top: int, full: number, unit: string, colors: record]: record -> list<record> {
    let row = $in
    let filled = if $row.value == 0 {
        0
    } else {
        $TRACK_WIDTH * ($row.value / $full)
        | math round --precision 1
        | [$in, $MIN_BAR_WIDTH]
        | math max
    }
    let bar_top = $top + ($ROW_HEIGHT - $BAR_HEIGHT) // 2
    let baseline = $top + $ROW_HEIGHT - $DESCENT
    let style = { y: $baseline, fill: $colors.text, font-weight: $row.weight }
    let notes = if $row.note == null {
        []
    } else {
        [($row.note | note-node $baseline $filled $colors)]
    }

    [
        (node text { x: 0, ...$style, font-size: 14, font-family: $FONT } [$row.name])
        (bar $bar_top $TRACK_WIDTH $colors.track)
        (bar $bar_top $filled ($colors | get $row.name))
        ...$notes
        (
            node text {
                x: ($LABEL_WIDTH + $TRACK_WIDTH + $VALUE_GAP)
                ...$style
                font-size: 13
                font-family: $MONO
            } [
                ($row.value | amount $unit)
            ]
        )
    ]
}

def note-node [baseline: int, filled: number, colors: record]: string -> record {
    let note = $in
    let fits = ($note | str length) * $NOTE_CHAR_WIDTH + 2 * $NOTE_PADDING <= $TRACK_WIDTH - $filled
    let place = if $fits {
        { x: ($LABEL_WIDTH + $filled + $NOTE_PADDING), fill: $colors.nuten }
    } else {
        { x: ($LABEL_WIDTH + $filled - $NOTE_PADDING), fill: $colors.on_nuten, text-anchor: end }
    }

    node text { y: $baseline, font-size: 13, font-weight: bold, font-family: $FONT, ...$place } [
        $note
    ]
}

def bar [y: number, width: number, fill: string]: nothing -> record {
    node rect { x: $LABEL_WIDTH, y: $y, width: $width, height: $BAR_HEIGHT, rx: 3, fill: $fill }
}

def node [tag: string, attributes: record, content: list = []]: nothing -> record {
    {
        tag: $tag
        attributes: (
            $attributes | items {|name, value| [$name, ($value | into string)] } | into record
        )
        content: $content
    }
}

def description [groups: list<record>]: nothing -> string {
    $groups
    | each {|group|
          $"($group.name), ($group.detail): nuten ($group.nuten | amount $group.unit), nufmt ($group.nufmt | amount $group.unit)"
      }
    | str join '. '
}

def amount [unit: string]: number -> string {
    let value = $in

    match $unit {
        nanoseconds => ($value / 1_000_000 | milliseconds)
        bytes => $"($value / 1_048_576 | fixed 1) MiB"
        files => $"($value | group-digits) (if $value == 1 { 'file' } else { 'files' })"
        _ => { error make { msg: $"no way to write an amount in ($unit)" } }
    }
}

def milliseconds []: number -> string {
    let ms = $in
    let digits = if ($ms | math round --precision 1) < 10 { 1 } else { 0 }

    $"($ms | fixed $digits) ms"
}

def fixed [digits: int]: number -> string {
    into string --decimals $digits
}

# Nushell groups digits by the locale, which would make the chart differ from machine to machine.
def group-digits []: int -> string {
    into string
    | split chars
    | reverse
    | chunks 3
    | each { reverse | str join }
    | reverse
    | str join ','
}
