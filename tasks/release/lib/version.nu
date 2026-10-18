const SEMVER = '^(?<major>0|[1-9][0-9]*)\.(?<minor>0|[1-9][0-9]*)\.(?<patch>0|[1-9][0-9]*)$'

export const PLACEHOLDER = '{version}'
export const UNRELEASED = '0.0.0'

export def bump [version: string, part: string]: nothing -> string {
    let current = parts $version

    match $part {
        major => [($current.major + 1), 0, 0]
        minor => [$current.major, ($current.minor + 1), 0]
        patch => [$current.major, $current.minor, ($current.patch + 1)]
        _ => { error make { msg: $"($part) is not one of major, minor, patch" } }
    }
    | str join '.'
}

export def newer [version: string, than: string]: nothing -> bool {
    let left = parts $version
    let right = parts $than
    let differences = [major, minor, patch]
        | each {|part| ($left | get $part) - ($right | get $part) }
        | where $it != 0

    not ($differences | is-empty) and ($differences | first) > 0
}

export def newest []: list<string> -> oneof<string, nothing> {
    let versions = $in
    if ($versions | is-empty) {
        return null
    }

    $versions | reduce {|version, latest|
        if (newer $version $latest) { $version } else { $latest }
    }
}

export def fill [template: string, version: string]: nothing -> string {
    $template | str replace $PLACEHOLDER $version
}

export def extract [template: string, text: string]: nothing -> oneof<string, nothing> {
    let around = $template | split row $PLACEHOLDER
    let before = $around | first
    let after = $around | last
    let fits = ($text | str starts-with $before) and ($text | str ends-with $after)
    let end = ($text | str length) - ($after | str length)
    let found = if $fits { $text | str substring ($before | str length)..<$end } else { '' }

    if $found =~ $SEMVER {
        $found
    } else {
        null
    }
}

def parts [version: string]: nothing -> record<major: int, minor: int, patch: int> {
    let found = $version | parse --regex $SEMVER
    if ($found | is-empty) {
        error make { msg: $"($version) is not a version of the form 1.2.3" }
    }

    $found | first | into int major minor patch
}
