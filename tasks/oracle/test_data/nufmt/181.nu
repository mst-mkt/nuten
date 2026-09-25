# https://github.com/nushell/nufmt/issues/181

def f [path] {
    tp $path e>| /dev/null
    tp $path o+e>| /dev/null
}
