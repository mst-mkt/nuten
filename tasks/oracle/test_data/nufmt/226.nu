# https://github.com/nushell/nufmt/issues/226

def to-names [] {
    match ($in | describe) {
        "string" => ($in | from json)
        _ => ($in | get name)
    }
}
