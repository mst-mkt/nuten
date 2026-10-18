use places.nu
use version.nu

export def name [group: record, version: string]: nothing -> string {
    version fill $group.tag $version
}

export def identify [
    groups: table
    tag: string
]: nothing -> oneof<record<group: string, version: string>, nothing> {
    $groups
    | each {|group|
          let found = version extract $group.tag $tag

          if $found != null { { group: $group.name, version: $found } }
      }
    | get 0?
}

export def released [group: record, tags: list<string>]: nothing -> oneof<string, nothing> {
    $tags | each {|tag| version extract $group.tag $tag } | version newest
}

export def due [current: string, released: oneof<string, nothing>]: nothing -> bool {
    version newer $current ($released | default $version.UNRELEASED)
}

export def due-tag [
    group: record
    texts: table<file: string, text: any>
    tags: list<string>
]: nothing -> oneof<string, nothing> {
    let current = places agreed $group $texts

    if (due $current (released $group $tags)) {
        name $group $current
    }
}
