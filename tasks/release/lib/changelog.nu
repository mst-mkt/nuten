const SECTIONS = [
    { type: feat, title: Features }
    { type: add, title: Additions }
    { type: fix, title: Fixes }
    { type: perf, title: Performance }
]

const SUBJECT = '^(?<type>[a-z]+)(?:\((?<scope>[^)]*)\))?(?<breaking>!)?: (?<summary>.+)$'

export def entries []: list<string> -> table<type: string, scope: string, breaking: bool, summary: string> {
    $in
    | each { parse --regex $SUBJECT }
    | flatten
    | update scope { default '' }
    | update breaking { $in == '!' }
    | where { $in.breaking or $in.type in ($SECTIONS | get type) }
}

export def body []: table<type: string, scope: string, breaking: bool, summary: string> -> string {
    let entries = $in
    let breaking = $entries | where breaking | each { item }
    let typed = $SECTIONS
        | each {|section|
              {
                  title: $section.title
                  items: ($entries | where not breaking and type == $section.type | each { item })
              }
          }
    let subsections = [{ title: 'Breaking changes', items: $breaking }, ...$typed]
        | where not ($it.items | is-empty)
        | each { [$"### ($in.title)", '', ...$in.items] | str join "\n" }

    if ($subsections | is-empty) {
        'No notable changes'
    } else {
        $subsections | str join "\n\n"
    }
}

export def section [version: string, date: string, body: string]: nothing -> string {
    $"## ($version) - ($date)\n\n($body)\n"
}

export def prepend [section: string]: oneof<string, nothing> -> string {
    let text = $in | default ''
    let lines = $text | split row "\n"
    let newest = $lines | enumerate | where { $in.item | str starts-with '## ' } | get 0?.index

    if ($text | str trim | is-empty) {
        $"# Changelog\n\n($section)"
    } else if $newest == null {
        $"($text | str trim --right)\n\n($section)"
    } else {
        [...($lines | take $newest), ...($section | split row "\n"), ...($lines | skip $newest)]
        | str join "\n"
    }
}

export def notes [version: string]: string -> string {
    let lines = $in | str replace --all "\r\n" "\n" | split row "\n"
    let headings = $lines | enumerate | where { $in.item | str starts-with '## ' }

    let start = $headings
        | where { ($in.item | str starts-with $"## ($version) ") or $in.item == $"## ($version)" }
        | get 0?.index
    if $start == null {
        error make { msg: $"the changelog has no section for ($version)" }
    }

    let end = $headings | where index > $start | get 0?.index | default ($lines | length)

    $lines | slice ($start + 1)..<($end) | str join "\n" | str trim
}

def item []: record -> string {
    if ($in.scope | is-empty) { $"- ($in.summary)" } else { $"- **($in.scope):** ($in.summary)" }
}
