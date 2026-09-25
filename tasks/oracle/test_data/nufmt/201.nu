# https://github.com/nushell/nufmt/issues/201

def main [] {
    let x = '"'
    print $"Hello: ($x)"
}
