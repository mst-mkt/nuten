# https://github.com/nushell/nufmt/issues/95

def do-thing [--check] { $check }
def go [--check] {
    do-thing --check=(not $check)
}
