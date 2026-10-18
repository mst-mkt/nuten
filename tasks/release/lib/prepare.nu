use changelog.nu
use places.nu
use tag.nu
use version.nu

const INITIAL = '🎉 Initial release'

export def files [group: record]: nothing -> list<string> {
    [...(places files $group), $group.changelog]
}

export def main [
    group: record
    texts: table<file: string, text: any>
    released: oneof<string, nothing>
    subjects: list<string>
    part: string
    date: string
]: nothing -> record<version: string, tag: string, writes: table<file: string, text: string>, locks: list<string>> {
    let current = places agreed $group $texts
    # A second bump before the first is tagged would skip a version and mix two releases into one section.
    if $current != ($released | default $version.UNRELEASED) {
        error make { msg: $"($group.name) is at ($current), which has no tag; merge or close the release prepared for it first" }
    }

    let next = version bump $current $part
    let body = if $released == null {
        $INITIAL
    } else {
        $subjects | changelog entries | changelog body
    }
    let section = changelog section $next $date $body
    let prepended = places text $texts $group.changelog | changelog prepend $section

    {
        version: $next
        tag: (tag name $group $next)
        writes: [
            ...(bumped $group $texts $current $next)
            { file: $group.changelog, text: $prepended }
        ]
        locks: $group.locks
    }
}

def bumped [
    group: record
    texts: table<file: string, text: any>
    from: string
    to: string
]: nothing -> table<file: string, text: string> {
    $group.versions
    | group-by file --to-table
    | each {|locations|
          let original = places existing-text $texts $locations.file
          let rewritten = $locations.items
              | reduce --fold $original {|location, text| places rewrite $location $text $from $to }

          { file: $locations.file, text: $rewritten }
      }
}
