# Edit this code to see how Nuten formats it.

def greet [name: string, --loud (-l)] {
let message = $"hello, ($name)"
  if $loud {$message | str upcase} else {$message}
}

ls|where size > 1kb|sort-by modified --reverse|each {|file| {name: $file.name, size: $file.size}}

let config = {
theme: dark, tabs: [general editor]
}
