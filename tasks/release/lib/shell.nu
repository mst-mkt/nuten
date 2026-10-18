export def root []: nothing -> path {
    ^git rev-parse --show-toplevel | str trim
}

export def read-files [root: path, files: list<string>]: nothing -> table<file: string, text: any> {
    $files
    | each {|file|
          let path = $root | path join $file

          { file: $file, text: (if ($path | path exists) { open --raw $path }) }
      }
}

export def write-files [root: path, texts: table<file: string, text: string>]: nothing -> nothing {
    $texts
    | each {|written| $written.text | save --force --raw ($root | path join $written.file) }
    | ignore
}

export def subjects [root: path, since: string, paths: list<string>]: nothing -> list<string> {
    ^git -C $root log --no-merges --reverse --format=%s $"($since)..HEAD" -- ...$paths | lines
}

export def tags [root: path]: nothing -> list<string> {
    ^git -C $root tag --list | lines
}

export def create-tag [root: path, name: string]: nothing -> nothing {
    ^git -C $root tag --annotate --message $"Release ($name)" $name
}

# Cargo keeps the format of its locks, so it writes them rather than this task.
export def update-locks [root: path, locks: list<string>]: nothing -> nothing {
    $locks
    | each {|lock|
          ^cargo update --workspace --manifest-path (
              $root | path join $lock | path dirname | path join Cargo.toml
          )
      }
    | ignore
}
