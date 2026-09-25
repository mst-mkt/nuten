# https://github.com/nushell/nufmt/issues/200

def f [] {
    "a" | {
        before: ($in | str trim | lines)
        after: ($in | split row (char --integer 0))
    }
}
