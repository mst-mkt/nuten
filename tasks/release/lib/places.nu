use version.nu

export def files [group: record]: nothing -> list<string> {
    [...($group.versions | get file), ...$group.locks] | uniq
}

export def text [
    texts: table<file: string, text: any>
    file: string
]: nothing -> oneof<string, nothing> {
    $texts | where file == $file | get 0?.text
}

export def existing-text [texts: table<file: string, text: any>, file: string]: nothing -> string {
    let found = text $texts $file
    if $found == null {
        error make { msg: $"($file) does not exist" }
    }

    $found
}

export def versions [
    group: record
    texts: table<file: string, text: any>
]: nothing -> table<file: string, field: string, version: string> {
    let written = $group.versions
        | each {|location|
              let held = version-at $location (existing-text $texts $location.file)

              { file: $location.file, field: ($location.field | str join '.'), version: $held }
          }
    let locked = $group.locks
        | each {|lock| locked (existing-text $texts $lock) | each {|held| { file: $lock, field: package, version: $held } } }
        | flatten

    [...$written, ...$locked]
}

export def agreed [group: record, texts: table<file: string, text: any>]: nothing -> string {
    let places = versions $group $texts
    if ($places | get version | uniq | length) != 1 {
        let held = $places | each { $"($in.file) \(($in.field)\) ($in.version)" } | str join ', '
        error make { msg: $"the places of ($group.name) disagree on its version: ($held)" }
    }

    $places | first | get version
}

# Read with the structure of the file, so a line that only looks like the version is never taken for it.
export def version-at [location: record, text: string]: nothing -> string {
    let parsed = match (file-kind $location.file) {
        json => ($text | from json)
        toml => ($text | from toml)
    }
    let value = $parsed | get ($location.field | into cell-path)

    let found = version extract (template $location) $value
    if $found == null {
        error make {
            msg: $"($location.file) holds ($value) at ($location.field | str join '.'), which is not (template $location)"
        }
    }

    $found
}

# Rewrite only the one line that holds the version, so the rest of the file keeps its form.
export def rewrite [location: record, text: string, from: string, to: string]: nothing -> string {
    let old = entry $location $from
    let pattern = '(?m)^([ \t]*)' + ($old | str replace --all --regex '([\\.^$|?*+()\[\]{}])' '\$1')

    let hits = $text | parse --regex $pattern | length
    if $hits != 1 {
        error make { msg: $"($location.file) has ($hits) lines that start with ($old), not 1" }
    }

    # The replacement reads $ as a group, so a $ of the entry is doubled.
    let replacement = $"${1}(entry $location $to | str replace --all '$' '$$')"
    let rewritten = $text | str replace --regex $pattern $replacement

    # The line may belong to another table, which only the structure tells.
    let written = version-at $location $rewritten
    if $written != $to {
        error make { msg: $"($location.file) holds ($written) at ($location.field | str join '.') once ($to) is written" }
    }

    $rewritten
}

# Cargo writes each package of the workspace into the lock without a source.
export def locked [text: string]: nothing -> list<string> {
    $text
    | from toml
    | get package
    | where source? == null
    | get version
    | uniq
}

def template [location: record]: nothing -> string {
    $location.value? | default $version.PLACEHOLDER
}

def file-kind [file: string]: nothing -> string {
    let extension = $file | path parse | get extension
    if $extension not-in [json, toml] {
        error make { msg: $"($file) is a .($extension) file, which has no known form for the version" }
    }

    $extension
}

def entry [location: record, version: string]: nothing -> string {
    let key = $location.field | last
    let value = version fill (template $location) $version

    match (file-kind $location.file) {
        json => $'"($key)": "($value)"'
        toml => $'($key) = "($value)"'
    }
}
