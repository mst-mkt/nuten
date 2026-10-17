export def stage [files: list<path>, corpus: path]: nothing -> path {
    let root = mktemp --directory
    $files
    | enumerate
    | each {|entry|
          let target = $root | path join (staged-path $entry.item $corpus $entry.index)
          mkdir ($target | path dirname)
          cp $entry.item $target
      }
    | ignore

    $root
}

def staged-path [file: path, corpus: path, index: int]: nothing -> path {
    let relative = if ($file | str starts-with $"($corpus)/") {
        $file | path relative-to $corpus
    } else {
        [also, ($index | into string), ($file | path basename)] | path join
    }

    $relative
    | path split
    | each { if ($in | str starts-with '.') { $"_($in | str substring 1..)" } else { $in } }
    | path join
}
